use integer_sqrt::IntegerSquareRoot;
use std::num::NonZeroU32;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, Result};
use bytes::{Bytes, BytesMut};
use chrono::Local;
use crossbeam_channel::{bounded, Receiver, Sender};
use pariter::{scope, IteratorExt};
use zstd::encode_all;

use super::BlobType;
use crate::backend::{DecryptFullBackend, DecryptWriteBackend, FileType};
use crate::crypto::{hash, CryptoKey};
use crate::id::Id;
use crate::index::SharedIndexer;
use crate::repofile::{
    ConfigFile, IndexBlob, IndexPack, PackHeaderLength, PackHeaderRef, SnapshotSummary,
};

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
    // This is a hack: raw_packer and indexer are only used in the add_raw() method.
    // TODO: Refactor as actor, like the other add() methods
    raw_packer: Arc<RwLock<RawPacker<BE>>>,
    indexer: SharedIndexer<BE>,
    sender: Sender<(Bytes, Id, Option<u32>)>,
    finish: Receiver<Result<PackerStats>>,
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

        let (tx, rx) = bounded(0);
        let (finish_tx, finish_rx) = bounded::<Result<PackerStats>>(0);
        let packer = Self {
            raw_packer: raw_packer.clone(),
            indexer: indexer.clone(),
            sender: tx,
            finish: finish_rx,
        };

        std::thread::spawn(move || {
            scope(|scope| {
                let status = rx
                    .into_iter()
                    .readahead_scoped(scope)
                    .filter(|(_, id, _)| !indexer.read().unwrap().has(id))
                    .filter(|(_, id, _)| !raw_packer.read().unwrap().has(id))
                    .readahead_scoped(scope)
                    .parallel_map_scoped(
                        scope,
                        |(data, id, size_limit): (Bytes, Id, Option<u32>)| {
                            let data_len: u32 = data.len().try_into()?;
                            let (data, uncompressed_length) = match zstd {
                                None => (
                                    key.encrypt_data(&data)
                                        .map_err(|_| anyhow!("crypto error"))?,
                                    None,
                                ),
                                // compress if requested
                                Some(level) => (
                                    key.encrypt_data(&encode_all(&*data, level)?)
                                        .map_err(|_| anyhow!("crypto error"))?,
                                    NonZeroU32::new(data_len),
                                ),
                            };
                            Ok((
                                data,
                                id,
                                u64::from(data_len),
                                uncompressed_length,
                                size_limit,
                            ))
                        },
                    )
                    .readahead_scoped(scope)
                    .try_for_each(|item: Result<_>| {
                        let (data, id, data_len, ul, size_limit) = item?;
                        raw_packer
                            .write()
                            .unwrap()
                            .add_raw(&data, &id, data_len, ul, size_limit)
                    })
                    .and_then(|_| raw_packer.write().unwrap().finalize());
                let _ = finish_tx.send(status);
            })
            .unwrap();
        });

        Ok(packer)
    }

    /// adds the blob to the packfile
    pub fn add(&self, data: Bytes, id: Id) -> Result<()> {
        // compute size limit based on total size and size bounds
        self.add_with_sizelimit(data, id, None)
    }

    /// adds the blob to the packfile, allows specifying a size limit for the pack file
    pub fn add_with_sizelimit(&self, data: Bytes, id: Id, size_limit: Option<u32>) -> Result<()> {
        self.sender.send((data, id, size_limit))?;
        Ok(())
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
        // cancel channel
        drop(self.sender);
        // wait for items in channel to be processed
        self.finish.recv().unwrap()
    }
}

#[derive(Default)]
pub struct PackerStats {
    pub blobs: u64,
    pub data: u64,
    pub data_packed: u64,
}

impl PackerStats {
    pub fn apply(self, summary: &mut SnapshotSummary, tpe: BlobType) {
        summary.data_added += self.data;
        summary.data_added_packed += self.data_packed;
        match tpe {
            BlobType::Tree => {
                summary.tree_blobs += self.blobs;
                summary.data_added_trees += self.data;
                summary.data_added_trees_packed += self.data_packed;
            }
            BlobType::Data => {
                summary.data_blobs += self.blobs;
                summary.data_added_files += self.data;
                summary.data_added_files_packed += self.data_packed;
            }
        }
    }
}

pub struct RawPacker<BE: DecryptWriteBackend> {
    be: BE,
    blob_type: BlobType,
    file: BytesMut,
    size: u32,
    count: u32,
    created: SystemTime,
    index: IndexPack,
    file_writer: Option<Actor>,
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

        // write file to backend
        let index = std::mem::take(&mut self.index);
        let file = std::mem::replace(&mut self.file, BytesMut::new());
        self.file_writer
            .as_ref()
            .unwrap()
            .send((file.into(), index))?;

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

impl<BE: DecryptWriteBackend> FileWriterHandle<BE> {
    fn process(&self, load: (Bytes, Id, IndexPack)) -> Result<IndexPack> {
        let (file, id, mut index) = load;
        index.id = id;
        self.be
            .write_bytes(FileType::Pack, &id, self.cacheable, file)?;
        index.time = Some(Local::now());
        Ok(index)
    }

    fn index(&self, index: IndexPack) -> Result<()> {
        self.indexer.write().unwrap().add(index)?;
        Ok(())
    }
}

pub struct Actor {
    sender: Sender<(Bytes, IndexPack)>,
    finish: Receiver<Result<()>>,
}

impl Actor {
    fn new<BE: DecryptWriteBackend>(
        fwh: FileWriterHandle<BE>,
        queue_len: usize,
        _par: usize,
    ) -> Self {
        let (tx, rx) = bounded(queue_len);
        let (finish_tx, finish_rx) = bounded::<Result<()>>(0);

        std::thread::spawn(move || {
            scope(|scope| {
                let status = rx
                    .into_iter()
                    .readahead_scoped(scope)
                    .map(|(file, index): (Bytes, IndexPack)| {
                        let id = hash(&file);
                        (file, id, index)
                    })
                    .readahead_scoped(scope)
                    .map(|load| fwh.process(load))
                    .readahead_scoped(scope)
                    .try_for_each(|index| fwh.index(index?));
                let _ = finish_tx.send(status);
            })
            .unwrap();
        });

        Self {
            sender: tx,
            finish: finish_rx,
        }
    }

    pub fn send(&self, load: (Bytes, IndexPack)) -> Result<()> {
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
            .add_with_sizelimit(data, blob.id, Some(self.size_limit))?;
        Ok(())
    }

    pub fn finalize(self) -> Result<PackerStats> {
        self.packer.finalize()
    }
}
