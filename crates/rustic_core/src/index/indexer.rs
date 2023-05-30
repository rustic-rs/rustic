use std::{
    collections::HashSet,
    sync::{Arc, RwLock},
    time::SystemTime,
};

use crate::{
    backend::decrypt::DecryptWriteBackend,
    error::IndexErrorKind,
    id::Id,
    repofile::indexfile::{IndexFile, IndexPack},
    RusticResult,
};
pub(super) mod constants {
    use std::time::Duration;
    pub(super) const MAX_COUNT: usize = 50_000;
    pub(super) const MAX_AGE: Duration = Duration::from_secs(300);
}

pub(crate) type SharedIndexer<BE> = Arc<RwLock<Indexer<BE>>>;

#[derive(Debug)]
pub struct Indexer<BE>
where
    BE: DecryptWriteBackend,
{
    be: BE,
    file: IndexFile,
    count: usize,
    created: SystemTime,
    indexed: Option<HashSet<Id>>,
}

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

    pub fn finalize(&self) -> RusticResult<()> {
        self.save()
    }

    pub fn save(&self) -> RusticResult<()> {
        if (self.file.packs.len() + self.file.packs_to_delete.len()) > 0 {
            _ = self.be.save_file(&self.file)?;
        }
        Ok(())
    }

    pub fn add(&mut self, pack: IndexPack) -> RusticResult<()> {
        self.add_with(pack, false)
    }

    pub fn add_remove(&mut self, pack: IndexPack) -> RusticResult<()> {
        self.add_with(pack, true)
    }

    pub fn add_with(&mut self, pack: IndexPack, delete: bool) -> RusticResult<()> {
        self.count += pack.blobs.len();

        if let Some(indexed) = &mut self.indexed {
            for blob in &pack.blobs {
                _ = indexed.insert(blob.id);
            }
        }

        self.file.add(pack, delete);

        // check if IndexFile needs to be saved
        if self.count >= constants::MAX_COUNT
            || self
                .created
                .elapsed()
                .map_err(IndexErrorKind::CouldNotGetElapsedTimeFromSystemTime)?
                >= constants::MAX_AGE
        {
            self.save()?;
            self.reset();
        }
        Ok(())
    }

    pub fn has(&self, id: &Id) -> bool {
        self.indexed
            .as_ref()
            .map_or(false, |indexed| indexed.contains(id))
    }
}
