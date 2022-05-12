use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, Result};
use binrw::{io::Cursor, BinWrite};
use chrono::Local;
use tempfile::tempfile;
use tokio::{spawn, task::JoinHandle};

use super::BlobType;
use crate::backend::{DecryptWriteBackend, FileType};
use crate::crypto::{CryptoKey, Hasher};
use crate::id::Id;
use crate::index::SharedIndexer;
use crate::repo::IndexPack;

const KB: u32 = 1024;
const MB: u32 = 1024 * KB;
const MAX_SIZE: u32 = 4 * MB;
const MAX_COUNT: u32 = 10_000;
const MAX_AGE: Duration = Duration::from_secs(300);

pub struct Packer<BE: DecryptWriteBackend> {
    be: BE,
    file: File,
    size: u32,
    count: u32,
    created: SystemTime,
    index: IndexPack,
    indexer: SharedIndexer<BE>,
    hasher: Hasher,
    file_writer: FileWriter<BE>,
}

impl<BE: DecryptWriteBackend> Packer<BE> {
    pub fn new(be: BE, indexer: SharedIndexer<BE>) -> Result<Self> {
        let file_writer = FileWriter {
            future: None,
            be: be.clone(),
            indexer: indexer.clone(),
        };
        Ok(Self {
            be,
            file: tempfile()?,
            size: 0,
            count: 0,
            created: SystemTime::now(),
            index: IndexPack::default(),
            indexer,
            hasher: Hasher::new(),
            file_writer,
        })
    }

    pub async fn finalize(&mut self) -> Result<()> {
        self.save().await?;
        self.file_writer.finalize().await
    }

    pub async fn write_data(&mut self, data: &[u8]) -> Result<u32> {
        self.hasher.update(data);
        let len = self.file.write(data)?.try_into()?;
        self.size += len;
        Ok(len)
    }

    // adds the blob to the packfile; returns false if it is already contained
    pub async fn add(&mut self, data: &[u8], id: &Id, tpe: BlobType) -> Result<bool> {
        // only add if this blob is not present
        if self.has(id) {
            return Ok(false);
        }
        {
            let indexer = self.indexer.read().await;
            if indexer.has(id) {
                return Ok(false);
            }
        }
        let data = self
            .be
            .key()
            .encrypt_data(data)
            .map_err(|_| anyhow!("crypto error"))?;

        self.add_raw(&data, id, tpe).await?;
        Ok(true)
    }

    // adds the already encrypted blob to the packfile without any check
    pub async fn add_raw(&mut self, data: &[u8], id: &Id, tpe: BlobType) -> Result<()> {
        let offset = self.size;
        let len = self.write_data(data).await?;
        self.index.add(*id, tpe, offset, len);
        self.count += 1;

        // check if PackFile needs to be saved
        if self.count >= MAX_COUNT || self.size >= MAX_SIZE || self.created.elapsed()? >= MAX_AGE {
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
            tpe: BlobType,
            #[bw(little)]
            len: u32,
            id: Id,
        }

        // collect header entries
        let mut writer = Cursor::new(Vec::new());
        for blob in &self.index.blobs {
            PackHeaderEntry {
                tpe: blob.tpe,
                len: blob.length,
                id: blob.id,
            }
            .write_to(&mut writer)?;
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
        self.file.flush()?;
        self.file.seek(SeekFrom::Start(0))?;

        let index = std::mem::take(&mut self.index);
        let file = std::mem::replace(&mut self.file, tempfile()?);

        self.file_writer.add(index, file, id).await?;

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
}

impl<BE: DecryptWriteBackend> FileWriter<BE> {
    async fn add(&mut self, mut index: IndexPack, file: File, id: Id) -> Result<()> {
        if let Some(fut) = self.future.take() {
            fut.await??;
        }

        let be = self.be.clone();
        let indexer = self.indexer.clone();
        self.future = Some(spawn(async move {
            be.write_file(FileType::Pack, &id, file).await?;
            index.time = Some(Local::now());
            indexer.write().await.add(index).await?;
            Ok(())
        }));

        Ok(())
    }

    async fn finalize(&mut self) -> Result<()> {
        if let Some(fut) = self.future.take() {
            return fut.await?;
        }
        Ok(())
    }
}
