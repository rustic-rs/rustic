use integer_sqrt::IntegerSquareRoot;
use std::num::NonZeroU32;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, Result};
use bytes::{Bytes, BytesMut};
use chrono::Local;
use crossbeam_channel::{bounded, Receiver, Sender};
use zstd::encode_all;

use super::BlobType;
use crate::backend::{DecryptFullBackend, DecryptWriteBackend, FileType};
use crate::crypto::{CryptoKey, Hasher};
use crate::id::Id;
use crate::index::SharedIndexer;
use crate::repofile::{ConfigFile, IndexBlob, IndexPack, PackHeaderLength, PackHeaderRef};

const KB: u32 = 1024;
const MB: u32 = 1024 * KB;
// the absolute maximum size of a pack: including headers it should not exceed 4 GB
const MAX_SIZE: u32 = 4076 * MB;
const MAX_COUNT: u32 = 10_000;
const MAX_AGE: Duration = Duration::from_secs(300);

pub struct PackSizer {
    default_size: u32,
    grow_factor: u32,
    size_limit: u32,
    current_size: u64,
    min_packsize_tolerate_percent: u32,
    max_packsize_tolerate_percent: u32,
}

impl PackSizer {
    pub fn from_config(config: &ConfigFile, blob_type: BlobType, current_size: u64) -> Self {
        let (default_size, grow_factor, size_limit) = config.packsize(blob_type);
        let (min_packsize_tolerate_percent, max_packsize_tolerate_percent) =
            config.packsize_ok_percents();
        Self {
            default_size,
            grow_factor,
            size_limit,
            current_size,
            min_packsize_tolerate_percent,
            max_packsize_tolerate_percent,
        }
    }

    pub fn pack_size(&self) -> u32 {
        (self.current_size.integer_sqrt() as u32 * self.grow_factor + self.default_size)
            .min(self.size_limit)
            .min(MAX_SIZE)
    }

    // returns whether the given size is not too small or too large
    pub fn size_ok(&self, size: u32) -> bool {
        let target_size = self.pack_size();
        // Note: we cast to u64 so that no overflow can occur in the multiplications
        u64::from(size) * 100
            >= u64::from(target_size) * u64::from(self.min_packsize_tolerate_percent)
            && u64::from(size) * 100
                <= u64::from(target_size) * u64::from(self.max_packsize_tolerate_percent)
    }

    fn add_size(&mut self, added: u32) {
        self.current_size += u64::from(added);
    }
}

#[derive(Clone)]
pub struct Packer<BE: DecryptWriteBackend> {
    raw_packer: Arc<RwLock<RawPacker<BE>>>,
    key: BE::Key,
    zstd: Option<i32>,
    indexer: SharedIndexer<BE>,
}

impl<BE: DecryptWriteBackend> Packer<BE> {
    pub fn new(
        be: BE,
        blob_type: BlobType,
        indexer: SharedIndexer<BE>,
        config: &ConfigFile,
        total_size: u64,
    ) -> Result<Self> {
        let key = be.key().clone();
        let raw_packer = Arc::new(RwLock::new(RawPacker::new(
            be,
            blob_type,
            indexer.clone(),
            config,
            total_size,
        )?));
        let zstd = config.zstd()?;

        Ok(Self {
            raw_packer,
            key,
            zstd,
            indexer,
        })
    }

    /// adds the blob to the packfile
    pub fn add(&self, data: &[u8], id: &Id) -> Result<()> {
        // compute size limit based on total size and size bounds
        self.add_with_sizelimit(data, id, None)
    }

    /// adds the blob to the packfile, allows specifying a size limit for the pack file
    pub fn add_with_sizelimit(&self, data: &[u8], id: &Id, size_limit: Option<u32>) -> Result<()> {
        // only add if this blob is not present
        if self.indexer.read().unwrap().has(id) {
            // Note: This is within two if clauses , because here the indexer lock is already released.
            // using "if self.indexer.read().unwrap().has(id) || self.raw_packer.read().unwrap().has(id)"
            // can lead to a deadlock as the indexer lock is hold too long (and also needed within raw_packer!)
            if self.raw_packer.read().unwrap().has(id) {
                return Ok(());
            }
        }

        let key = self.key.clone();
        let zstd = self.zstd;

        let data_len: u32 = data.len().try_into()?;
        let (data, uncompressed_length) = match zstd {
            None => (
                key.encrypt_data(data)
                    .map_err(|_| anyhow!("crypto error"))?,
                None,
            ),
            // compress if requested
            Some(level) => (
                key.encrypt_data(&encode_all(data, level)?)
                    .map_err(|_| anyhow!("crypto error"))?,
                NonZeroU32::new(data_len),
            ),
        };
        self.add_raw(
            &data,
            id,
            u64::from(data_len),
            uncompressed_length,
            size_limit,
        )
    }

    /// adds the already encrypted (and maybe compressed) blob to the packfile
    pub fn add_raw(
        &self,
        data: &[u8],
        id: &Id,
        data_len: u64,
        uncompressed_length: Option<NonZeroU32>,
        size_limit: Option<u32>,
    ) -> Result<()> {
        // only add if this blob is not present
        if self.indexer.read().unwrap().has(id) {
            Ok(())
        } else {
            self.raw_packer.write().unwrap().add_raw(
                data,
                id,
                data_len,
                uncompressed_length,
                size_limit,
            )
        }
    }

    pub fn finalize(self) -> Result<PackerStats> {
        self.raw_packer.write().unwrap().finalize()
    }
}

#[derive(Default)]
pub struct PackerStats {
    pub blobs: u64,
    pub data: u64,
    pub data_packed: u64,
}

pub struct RawPacker<BE: DecryptWriteBackend> {
    be: BE,
    blob_type: BlobType,
    file: BytesMut,
    size: u32,
    count: u32,
    created: SystemTime,
    index: IndexPack,
    hasher: Hasher,
    file_writer: Option<Actor<(Bytes, Id, IndexPack)>>,
    pack_sizer: PackSizer,
    stats: PackerStats,
}

impl<BE: DecryptWriteBackend> RawPacker<BE> {
    pub fn new(
        be: BE,
        blob_type: BlobType,
        indexer: SharedIndexer<BE>,
        config: &ConfigFile,
        total_size: u64,
    ) -> Result<Self> {
        let file_writer = Some(Actor::new(
            FileWriterHandle {
                be: be.clone(),
                indexer,
                cacheable: blob_type.is_cacheable(),
            },
            1,
            1,
        ));
        let pack_sizer = PackSizer::from_config(config, blob_type, total_size);
        Ok(Self {
            be,
            blob_type,
            file: BytesMut::new(),
            size: 0,
            count: 0,
            created: SystemTime::now(),
            index: IndexPack::default(),
            hasher: Hasher::new(),
            file_writer,
            pack_sizer,
            stats: PackerStats::default(),
        })
    }

    pub fn finalize(&mut self) -> Result<PackerStats> {
        self.save()?;
        self.file_writer.take().unwrap().finalize()?;
        Ok(std::mem::take(&mut self.stats))
    }

    pub fn write_data(&mut self, data: &[u8]) -> Result<u32> {
        self.hasher.update(data);
        let len = data.len().try_into()?;
        self.file.extend_from_slice(data);
        self.size += len;
        Ok(len)
    }

    // adds the already compressed/encrypted blob to the packfile without any check
    pub fn add_raw(
        &mut self,
        data: &[u8],
        id: &Id,
        data_len: u64,
        uncompressed_length: Option<NonZeroU32>,
        size_limit: Option<u32>,
    ) -> Result<()> {
        if self.has(id) {
            return Ok(());
        }

        self.stats.blobs += 1;
        self.stats.data += data_len;
        let data_len_packed: u64 = data.len().try_into()?;
        self.stats.data_packed += data_len_packed;

        let size_limit = size_limit.unwrap_or_else(|| self.pack_sizer.pack_size());
        let offset = self.size;
        let len = self.write_data(data)?;
        self.index
            .add(*id, self.blob_type, offset, len, uncompressed_length);
        self.count += 1;

        // check if PackFile needs to be saved
        if self.count >= MAX_COUNT || self.size >= size_limit || self.created.elapsed()? >= MAX_AGE
        {
            self.pack_sizer.add_size(self.index.pack_size());
            self.save()?;
            self.size = 0;
            self.count = 0;
            self.created = SystemTime::now();
            self.hasher.reset();
        }
        Ok(())
    }

    /// writes header and length of header to packfile
    pub fn write_header(&mut self) -> Result<()> {
        // comput the pack header
        let data = PackHeaderRef::from_index_pack(&self.index).to_binary()?;

        // encrypt and write to pack file
        let data = self
            .be
            .key()
            .encrypt_data(&data)
            .map_err(|_| anyhow!("crypto error"))?;

        let headerlen = data.len().try_into()?;
        self.write_data(&data)?;

        // finally write length of header unencrypted to pack file
        self.write_data(&PackHeaderLength::from_u32(headerlen).to_binary()?)?;

        Ok(())
    }

    pub fn save(&mut self) -> Result<()> {
        if self.size == 0 {
            return Ok(());
        }

        self.write_header()?;

        // compute id of packfile
        let id = self.hasher.finalize();
        self.index.set_id(id);

        // write file to backend
        let index = std::mem::take(&mut self.index);
        let file = std::mem::replace(&mut self.file, BytesMut::new());
        self.file_writer
            .as_ref()
            .unwrap()
            .send((file.into(), id, index))?;

        Ok(())
    }

    fn has(&self, id: &Id) -> bool {
        self.index.blobs.iter().any(|b| &b.id == id)
    }
}

#[derive(Clone)]
struct FileWriterHandle<BE: DecryptWriteBackend> {
    be: BE,
    indexer: SharedIndexer<BE>,
    cacheable: bool,
}

impl<BE: DecryptWriteBackend> ActorHandle<(Bytes, Id, IndexPack)> for FileWriterHandle<BE> {
    fn process(&self, load: (Bytes, Id, IndexPack)) -> Result<()> {
        let (file, id, mut index) = load;
        self.be
            .write_bytes(FileType::Pack, &id, self.cacheable, file)?;
        index.time = Some(Local::now());
        self.indexer.write().unwrap().add(index)?;
        Ok(())
    }
}

pub trait ActorHandle<T>: Clone + Send + 'static {
    fn process(&self, load: T) -> Result<()>;
}

pub struct Actor<T> {
    sender: Sender<T>,
    finish: Receiver<Result<()>>,
}

impl<T: Send + Sync + 'static> Actor<T> {
    pub fn new(fwh: impl ActorHandle<T>, queue_len: usize, par: usize) -> Self {
        let (tx, rx) = bounded(queue_len);
        let (finish_tx, finish_rx) = bounded::<Result<()>>(0);
        (0..par).for_each(|_| {
            let rx = rx.clone();
            let finish_tx = finish_tx.clone();
            let fwh = fwh.clone();
            std::thread::spawn(move || {
                let mut status = Ok(());
                for load in rx {
                    // only keep processing if there was no error
                    if status.is_ok() {
                        status = fwh.process(load);
                    }
                }
                let _ = finish_tx.send(status);
            });
        });

        Self {
            sender: tx,
            finish: finish_rx,
        }
    }

    pub fn send(&self, load: T) -> Result<()> {
        self.sender.send(load)?;
        Ok(())
    }

    pub fn finalize(self) -> Result<()> {
        // cancel channel
        drop(self.sender);
        // wait for items in channel to be processed
        self.finish.recv().unwrap()
    }
}

pub struct Repacker<BE: DecryptFullBackend> {
    be: BE,
    packer: Packer<BE>,
    size_limit: u32,
}

impl<BE: DecryptFullBackend> Repacker<BE> {
    pub fn new(
        be: BE,
        blob_type: BlobType,
        indexer: SharedIndexer<BE>,
        config: &ConfigFile,
        total_size: u64,
    ) -> Result<Self> {
        let packer = Packer::new(be.clone(), blob_type, indexer, config, total_size)?;
        let size_limit = PackSizer::from_config(config, blob_type, total_size).pack_size();
        Ok(Self {
            be,
            packer,
            size_limit,
        })
    }

    pub fn add_fast(&self, pack_id: &Id, blob: &IndexBlob) -> Result<()> {
        let data = self.be.read_partial(
            FileType::Pack,
            pack_id,
            blob.tpe.is_cacheable(),
            blob.offset,
            blob.length,
        )?;
        self.packer.add_raw(
            &data,
            &blob.id,
            0,
            blob.uncompressed_length,
            Some(self.size_limit),
        )?;
        Ok(())
    }

    pub fn add(&self, pack_id: &Id, blob: &IndexBlob) -> Result<()> {
        let data = self.be.read_encrypted_partial(
            FileType::Pack,
            pack_id,
            blob.tpe.is_cacheable(),
            blob.offset,
            blob.length,
            blob.uncompressed_length,
        )?;
        self.packer
            .add_with_sizelimit(&data, &blob.id, Some(self.size_limit))?;
        Ok(())
    }

    pub fn finalize(self) -> Result<PackerStats> {
        self.packer.finalize()
    }
}
