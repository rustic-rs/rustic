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
    indexed: Option<HashSet<Id>>,
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
            indexed: Some(HashSet::new()),
        }
    }

    pub fn new_unindexed(be: BE) -> Self {
        Self {
            be,
            file: IndexFile::new(),
            count: 0,
            created: SystemTime::now(),
            indexed: None,
        }
    }

    pub fn reset(&mut self) {
        self.file = IndexFile::new();
        self.count = 0;
        self.created = SystemTime::now();
    }

    pub async fn finalize(&self) -> Result<()> {
        self.save().await
    }

    pub async fn save(&self) -> Result<()> {
        if self.count > 0 {
            self.be.save_file(&self.file).await?;
        }
        Ok(())
    }

    pub async fn add(&mut self, pack: IndexPack) -> Result<()> {
        self.count += pack.blobs().len();

        if let Some(indexed) = &mut self.indexed {
            for blob in pack.blobs() {
                indexed.insert(*blob.id());
            }
        }

        self.file.add(pack);

        // check if IndexFile needs to be saved
        if self.count >= MAX_COUNT || self.created.elapsed()? >= MAX_AGE {
            self.save().await?;
            self.reset();
        }
        Ok(())
    }

    pub fn has(&self, id: &Id) -> bool {
        match &self.indexed {
            None => false,
            Some(indexed) => indexed.contains(id),
        }
    }
}
