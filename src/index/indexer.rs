use std::collections::HashSet;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::{Duration, SystemTime};

use anyhow::Result;

use crate::backend::DecryptWriteBackend;
use crate::id::Id;
use crate::repofile::{IndexFile, IndexPack};

pub type SharedIndexer<BE> = Arc<RwLock<Indexer<BE>>>;

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
            file: IndexFile::default(),
            count: 0,
            created: SystemTime::now(),
            indexed: Some(HashSet::new()),
        }
    }

    pub fn new_unindexed(be: BE) -> Self {
        Self {
            be,
            file: IndexFile::default(),
            count: 0,
            created: SystemTime::now(),
            indexed: None,
        }
    }

    pub fn reset(&mut self) {
        self.file = IndexFile::default();
        self.count = 0;
        self.created = SystemTime::now();
    }

    pub fn into_shared(self) -> SharedIndexer<BE> {
        Arc::new(RwLock::new(self))
    }

    pub fn finalize(&self) -> Result<()> {
        self.save()
    }

    pub fn save(&self) -> Result<()> {
        if (self.file.packs.len() + self.file.packs_to_delete.len()) > 0 {
            self.be.save_file(&self.file)?;
        }
        Ok(())
    }

    pub fn add(&mut self, pack: IndexPack) -> Result<()> {
        self.add_with(pack, false)
    }

    pub fn add_remove(&mut self, pack: IndexPack) -> Result<()> {
        self.add_with(pack, true)
    }

    pub fn add_with(&mut self, pack: IndexPack, delete: bool) -> Result<()> {
        self.count += pack.blobs.len();

        if let Some(indexed) = &mut self.indexed {
            for blob in &pack.blobs {
                indexed.insert(blob.id);
            }
        }

        self.file.add(pack, delete);

        // check if IndexFile needs to be saved
        if self.count >= MAX_COUNT || self.created.elapsed()? >= MAX_AGE {
            self.save()?;
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
