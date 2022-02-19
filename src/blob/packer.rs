use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, Result};
use binrw::{io::Cursor, BinWrite};
use tempfile::tempfile;

use super::BlobType;
use crate::backend::{DecryptWriteBackend, FileType};
use crate::crypto::{CryptoKey, Hasher};
use crate::id::Id;
use crate::index::SharedIndexer;
use crate::repo::IndexPack;

const MAX_SIZE: u32 = 50000;
const MAX_AGE: Duration = Duration::from_secs(300);

pub struct Packer<BE: DecryptWriteBackend, C: CryptoKey> {
    be: BE,
    file: File,
    count: u32,
    created: SystemTime,
    index: IndexPack,
    indexer: SharedIndexer<BE>,
    hasher: Hasher,
    key: C,
}

impl<BE: DecryptWriteBackend, C: CryptoKey> Packer<BE, C> {
    pub fn new(be: BE, indexer: SharedIndexer<BE>, key: C) -> Result<Self> {
        Ok(Self {
            be,
            file: tempfile()?,
            count: 0,
            created: SystemTime::now(),
            index: IndexPack::new(),
            indexer,
            hasher: Hasher::new(),
            key,
        })
    }

    pub fn reset(&mut self) -> Result<()> {
        self.file = tempfile()?;
        self.count = 0;
        self.created = SystemTime::now();
        self.hasher.reset();
        Ok(())
    }

    pub fn finalize(&mut self) -> Result<()> {
        self.save()
    }

    pub fn write_data(&mut self, data: &[u8]) -> Result<u32> {
        self.hasher.update(&data);
        let len = self.file.write(&data)?.try_into()?;
        self.count += len;
        Ok(len)
    }

    pub fn add(&mut self, data: &[u8], id: &Id, tpe: BlobType) -> Result<()> {
        // only add if this blob is not present
        if self.has(id) {
            return Ok(());
        }
        if self.indexer.borrow().has(id) {
            return Ok(());
        }

        let offset = self.count;
        let data = self
            .key
            .encrypt_data(data)
            .map_err(|_| anyhow!("crypto error"))?;
        let len = self.write_data(&data)?;
        self.index.add(*id, tpe, offset, len);

        // check if PackFile needs to be saved
        if self.count >= MAX_SIZE || self.created.elapsed()? >= MAX_AGE {
            self.save()?;
            self.reset()?;
        }
        Ok(())
    }

    /// writes header and length of header to packfile
    pub fn write_header(&mut self) -> Result<()> {
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
        for blob in self.index.blobs() {
            PackHeaderEntry {
                tpe: *blob.tpe(),
                len: *blob.length(),
                id: *blob.id(),
            }
            .write_to(&mut writer)?;
        }

        // encrypt and write to pack file
        let data = writer.into_inner();
        let data = self
            .key
            .encrypt_data(&data)
            .map_err(|_| anyhow!("crypto error"))?;
        let headerlen = data.len();
        self.write_data(&data)?;

        // finally write length of header unencrypted to pack file
        let mut writer = Cursor::new(Vec::new());
        PackHeaderLength(headerlen.try_into()?).write_to(&mut writer)?;
        let data = writer.into_inner();
        self.write_data(&data)?;

        Ok(())
    }

    pub fn save(&mut self) -> Result<()> {
        if self.count == 0 {
            return Ok(());
        }

        self.write_header()?;

        // compute id of packfile
        let id = self.hasher.finalize();
        self.index.set_id(id);

        // write file to backend
        self.file.flush()?;
        self.file.seek(SeekFrom::Start(0))?;
        self.be.write_full(FileType::Pack, &id, &mut self.file)?;

        let index = std::mem::replace(&mut self.index, IndexPack::new());
        self.indexer.borrow_mut().add(index)?;
        Ok(())
    }

    fn has(&self, id: &Id) -> bool {
        self.index.blobs().iter().any(|b| b.id() == id)
    }
}
