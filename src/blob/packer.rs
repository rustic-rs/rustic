use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, Result};
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

    pub fn save(&mut self) -> Result<()> {
        if self.count == 0 {
            return Ok(());
        }
        let id = self.hasher.finalize();
        self.index.set_id(id);

        self.file.flush()?;
        self.file.seek(SeekFrom::Start(0))?;
        self.be.write_full(FileType::Pack, &id, &mut self.file)?;

        let index = std::mem::replace(&mut self.index, IndexPack::new());
        self.indexer.borrow_mut().add(index)?;
        Ok(())
    }

    pub fn add(&mut self, data: &[u8], id: &Id, tpe: BlobType) -> Result<()> {
        // only add if this blob is not present
        if self.has(id) {
            return Ok(());
        }
        if self.indexer.borrow().has(id) {
            return Ok(());
        }

        let data = self
            .key
            .encrypt_data(data)
            .map_err(|_| anyhow!("crypto error"))?;

        self.hasher.update(&data);
        let len = self.file.write(&data)?.try_into()?;
        self.index.add(*id, tpe, self.count, len);
        self.count += len;

        // check if IndexFile needs to be saved
        if self.count >= MAX_SIZE || self.created.elapsed()? >= MAX_AGE {
            self.save()?;
            self.reset()?;
        }
        Ok(())
    }

    fn has(&self, id: &Id) -> bool {
        self.index.blobs().iter().find(|b| b.id() == id).is_some()
    }
}

/*
impl<BE: WriteBackend> Drop for Packer<BE> {
    fn drop(&mut self) {
        // ignore error when dropping Indexer
        let _ = self.finalize();
    }
}
*/
