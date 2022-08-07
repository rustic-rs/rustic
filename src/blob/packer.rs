use integer_sqrt::IntegerSquareRoot;
use std::num::NonZeroU32;
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, Result};
use binrw::{io::Cursor, BinWrite};
use bytes::{Bytes, BytesMut};
use chrono::Local;
use tokio::{spawn, task::JoinHandle};
use zstd::encode_all;

use super::BlobType;
use crate::backend::{DecryptFullBackend, DecryptWriteBackend, FileType};
use crate::crypto::{CryptoKey, Hasher};
use crate::id::Id;
use crate::index::SharedIndexer;
use crate::repo::{ConfigFile, IndexBlob, IndexPack};

const KB: u32 = 1024;
const MB: u32 = 1024 * KB;
// the absolute maximum size of a pack: including headers it should not exceed 4 GB
const MAX_SIZE: u32 = 4076 * MB;
const MAX_COUNT: u32 = 10_000;
const MAX_AGE: Duration = Duration::from_secs(300);

struct PackSizer {
    default_size: u32,
    grow_factor: u32,
    current_size: u64,
}

impl PackSizer {
    pub fn from_config(config: &ConfigFile, blob_type: BlobType, current_size: u64) -> Self {
        let (default_size, grow_factor) = config.packsize(blob_type);
        Self {
            default_size,
            grow_factor,
            current_size,
        }
    }

    pub fn pack_size(&self) -> u32 {
        (self.current_size.integer_sqrt() as u32 * self.grow_factor + self.default_size)
            .min(MAX_SIZE)
    }

    fn add_size(&mut self, added: u32) {
        self.current_size += added as u64;
    }
}
pub struct Packer<BE: DecryptWriteBackend> {
    be: BE,
    blob_type: BlobType,
    file: BytesMut,
    size: u32,
    count: u32,
    created: SystemTime,
    index: IndexPack,
    indexer: SharedIndexer<BE>,
    hasher: Hasher,
    file_writer: FileWriter<BE>,
    zstd: Option<i32>,
    pack_sizer: PackSizer,
}

impl<BE: DecryptWriteBackend> Packer<BE> {
    pub fn new(
        be: BE,
        blob_type: BlobType,
        indexer: SharedIndexer<BE>,
        config: &ConfigFile,
        total_size: u64,
    ) -> Result<Self> {
        let file_writer = FileWriter {
            future: None,
            be: be.clone(),
            indexer: indexer.clone(),
            cacheable: blob_type.is_cacheable(),
        };
        let zstd = config.zstd()?;
        let pack_sizer = PackSizer::from_config(config, blob_type, total_size);
        Ok(Self {
            be,
            blob_type,
            file: BytesMut::new(),
            size: 0,
            count: 0,
            created: SystemTime::now(),
            index: IndexPack::default(),
            indexer,
            hasher: Hasher::new(),
            file_writer,
            zstd,
            pack_sizer,
        })
    }

    pub async fn finalize(&mut self) -> Result<()> {
        self.save().await?;
        self.file_writer.finalize().await
    }

    pub async fn write_data(&mut self, data: &[u8]) -> Result<u32> {
        self.hasher.update(data);
        let len = data.len().try_into()?;
        self.file.extend_from_slice(data);
        self.size += len;
        Ok(len)
    }

    // adds the blob to the packfile; returns the actually added size
    pub async fn add(&mut self, data: &[u8], id: &Id) -> Result<u64> {
        // compute size limit based on total size and size bounds
        let size_limit = self.pack_sizer.pack_size();
        self.add_with_sizelimit(data, id, size_limit).await
    }

    // adds the blob to the packfile; returns the actually added size
    pub async fn add_with_sizelimit(
        &mut self,
        data: &[u8],
        id: &Id,
        size_limit: u32,
    ) -> Result<u64> {
        // only add if this blob is not present
        if self.has(id) {
            return Ok(0);
        }
        {
            let indexer = self.indexer.read().await;
            if indexer.has(id) {
                return Ok(0);
            }
        }

        // compress if requested
        let data_len: u32 = data.len().try_into()?;
        let key = self.be.key();

        let (data, uncompressed_length) = match self.zstd {
            None => (
                key.encrypt_data(data)
                    .map_err(|_| anyhow!("crypto error"))?,
                None,
            ),
            Some(level) => (
                key.encrypt_data(&encode_all(data, level)?)
                    .map_err(|_| anyhow!("crypto error"))?,
                NonZeroU32::new(data_len),
            ),
        };

        // add using current total_size as repo_size
        self.add_raw(&data, id, uncompressed_length, size_limit)
            .await?;
        Ok(data.len().try_into()?)
    }

    // adds the already compressed/encrypted blob to the packfile without any check
    pub async fn add_raw(
        &mut self,
        data: &[u8],
        id: &Id,
        uncompressed_length: Option<NonZeroU32>,
        size_limit: u32,
    ) -> Result<()> {
        let offset = self.size;
        let len = self.write_data(data).await?;
        self.index
            .add(*id, self.blob_type, offset, len, uncompressed_length);
        self.count += 1;

        // check if PackFile needs to be saved
        if self.count >= MAX_COUNT || self.size >= size_limit || self.created.elapsed()? >= MAX_AGE
        {
            self.pack_sizer.add_size(self.index.pack_size());
            self.save().await?;
            self.size = 0;
            self.count = 0;
            self.created = SystemTime::now();
            self.hasher.reset();
        }
        Ok(())
    }

    /// writes header and length of header to packfile
    pub async fn write_header(&mut self) -> Result<()> {
        #[derive(BinWrite)]
        struct PackHeaderLength(#[bw(little)] u32);

        #[derive(BinWrite)]
        struct PackHeaderEntry {
            tpe: u8,
            #[bw(little)]
            len: u32,
            id: Id,
        }

        #[derive(BinWrite)]
        struct PackHeaderEntryComp {
            tpe: u8,
            #[bw(little)]
            len: u32,
            #[bw(little)]
            len_data: u32,
            id: Id,
        }

        // collect header entries
        let mut writer = Cursor::new(Vec::new());
        for blob in &self.index.blobs {
            match blob.uncompressed_length {
                None => PackHeaderEntry {
                    tpe: match blob.tpe {
                        BlobType::Data => 0b00,
                        BlobType::Tree => 0b01,
                    },
                    len: blob.length,
                    id: blob.id,
                }
                .write_to(&mut writer)?,
                Some(len) => PackHeaderEntryComp {
                    tpe: match blob.tpe {
                        BlobType::Data => 0b10,
                        BlobType::Tree => 0b11,
                    },
                    len: blob.length,
                    len_data: len.get(),
                    id: blob.id,
                }
                .write_to(&mut writer)?,
            };
        }

        // encrypt and write to pack file
        let data = writer.into_inner();
        let data = self
            .be
            .key()
            .encrypt_data(&data)
            .map_err(|_| anyhow!("crypto error"))?;
        let headerlen = data.len();
        self.write_data(&data).await?;

        // finally write length of header unencrypted to pack file
        let mut writer = Cursor::new(Vec::new());
        PackHeaderLength(headerlen.try_into()?).write_to(&mut writer)?;
        let data = writer.into_inner();
        self.write_data(&data).await?;

        Ok(())
    }

    pub async fn save(&mut self) -> Result<()> {
        if self.size == 0 {
            return Ok(());
        }

        self.write_header().await?;

        // compute id of packfile
        let id = self.hasher.finalize();
        self.index.set_id(id);

        // write file to backend
        let index = std::mem::take(&mut self.index);
        let file = std::mem::replace(&mut self.file, BytesMut::new());
        self.file_writer.add(index, file.into(), id).await?;

        Ok(())
    }

    fn has(&self, id: &Id) -> bool {
        self.index.blobs.iter().any(|b| &b.id == id)
    }
}

struct FileWriter<BE: DecryptWriteBackend> {
    future: Option<JoinHandle<Result<()>>>,
    be: BE,
    indexer: SharedIndexer<BE>,
    cacheable: bool,
}

impl<BE: DecryptWriteBackend> FileWriter<BE> {
    async fn add(&mut self, mut index: IndexPack, file: Bytes, id: Id) -> Result<()> {
        let be = self.be.clone();
        let indexer = self.indexer.clone();
        let cacheable = self.cacheable;
        let new_future = spawn(async move {
            be.write_bytes(FileType::Pack, &id, cacheable, file).await?;
            index.time = Some(Local::now());
            indexer.write().await.add(index).await?;
            Ok(())
        });

        if let Some(fut) = self.future.replace(new_future) {
            fut.await??;
        }
        Ok(())
    }

    async fn finalize(&mut self) -> Result<()> {
        if let Some(fut) = self.future.take() {
            return fut.await?;
        }
        Ok(())
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
        let size_limit = packer.pack_sizer.pack_size();
        Ok(Self {
            be,
            packer,
            size_limit,
        })
    }

    pub async fn add_fast(&mut self, pack_id: &Id, blob: &IndexBlob) -> Result<()> {
        let data = self
            .be
            .read_partial(
                FileType::Pack,
                pack_id,
                blob.tpe.is_cacheable(),
                blob.offset,
                blob.length,
            )
            .await?;
        self.packer
            .add_raw(&data, &blob.id, blob.uncompressed_length, self.size_limit)
            .await?;
        Ok(())
    }

    pub async fn add(&mut self, pack_id: &Id, blob: &IndexBlob) -> Result<()> {
        let data = self
            .be
            .read_encrypted_partial(
                FileType::Pack,
                pack_id,
                blob.tpe.is_cacheable(),
                blob.offset,
                blob.length,
                blob.uncompressed_length,
            )
            .await?;
        self.packer
            .add_with_sizelimit(&data, &blob.id, self.size_limit)
            .await?;
        Ok(())
    }

    pub async fn finalize(&mut self) -> Result<()> {
        self.packer.finalize().await
    }
}
