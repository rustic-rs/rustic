use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use anyhow::Result;

use crate::backend::DecryptWriteBackend;
use crate::id::Id;
use crate::repo::{IndexFile, IndexPack};

pub type SharedIndexer<BE> = Rc<RefCell<Indexer<BE>>>;

pub struct Indexer<BE: DecryptWriteBackend> {
    be: BE,
    file: IndexFile,
    count: usize,
    created: SystemTime,
    indexed: HashSet<Id>,
}

const MAX_COUNT: usize = 50_000;
const MAX_AGE: Duration = Duration::from_secs(300);

impl<BE: DecryptWriteBackend> Indexer<BE> {
    pub fn new(be: BE) -> Self {
        Self {
            be,
            file: IndexFile::new(),
            count: 0,
            created: SystemTime::now(),
            indexed: HashSet::new(),
        }
    }

    pub fn reset(&mut self) {
        self.file = IndexFile::new();
        self.count = 0;
        self.created = SystemTime::now();
    }

    pub fn finalize(&self) -> Result<()> {
        self.save()
    }

    pub fn save(&self) -> Result<()> {
        if self.count > 0 {
            self.file.save_to_backend(&self.be)?;
        }
        Ok(())
    }

    pub fn add(&mut self, pack: IndexPack) -> Result<()> {
        self.count += pack.blobs().len();

        for blob in pack.blobs() {
            self.indexed.insert(*blob.id());
        }

        self.file.add(pack);

        // check if IndexFile needs to be saved
        if self.count >= MAX_COUNT || self.created.elapsed()? >= MAX_AGE {
            self.save()?;
            self.reset();
        }
        Ok(())
    }

    pub fn has(&self, id: &Id) -> bool {
        self.indexed.contains(id)
    }
}

/*
impl<BE: WriteBackend> Drop for Indexer<BE> {
    fn drop(&mut self) {
        // ignore error when dropping Indexer
        let _ = self.save();
    }
}
*/
/*
impl<BE: WriteBackend> ReadIndex for Indexer<BE> {
    fn get_id(&self, id: &Id) -> Option<IndexEntry> {
        for pack in self.file.packs() {
            if let Some(blob) = pack.blobs().iter().find(|b| b.id() == id) {
                return Some(IndexEntry {
                    pack: *pack.id(),
                    tpe: *blob.tpe(),
                    offset: *blob.offset(),
                    length: *blob.length(),
                });
            }
        }
        None
    }
}
*/
