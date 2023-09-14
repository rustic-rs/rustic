use std::{
    collections::HashSet,
    sync::{Arc, RwLock},
    time::SystemTime,
};

use crate::{
    backend::decrypt::DecryptWriteBackend,
    error::{IndexErrorKind, RusticResult},
    id::Id,
    repofile::indexfile::{IndexFile, IndexPack},
};

pub(super) mod constants {
    use std::time::Duration;

    /// The maximum number of blobs to index before saving the index.
    pub(super) const MAX_COUNT: usize = 50_000;
    /// The maximum age of an index before saving the index.
    pub(super) const MAX_AGE: Duration = Duration::from_secs(300);
}

pub(crate) type SharedIndexer<BE> = Arc<RwLock<Indexer<BE>>>;

/// The `Indexer` is responsible for indexing blobs.
#[derive(Debug)]
pub struct Indexer<BE>
where
    BE: DecryptWriteBackend,
{
    /// The backend to write to.
    be: BE,
    /// The index file.
    file: IndexFile,
    /// The number of blobs indexed.
    count: usize,
    /// The time the indexer was created.
    created: SystemTime,
    /// The set of indexed blob ids.
    indexed: Option<HashSet<Id>>,
}

impl<BE: DecryptWriteBackend> Indexer<BE> {
    /// Creates a new `Indexer`.
    ///
    /// # Type Parameters
    ///
    /// * `BE` - The backend type.
    ///
    /// # Arguments
    ///
    /// * `be` - The backend to write to.
    pub fn new(be: BE) -> Self {
        Self {
            be,
            file: IndexFile::default(),
            count: 0,
            created: SystemTime::now(),
            indexed: Some(HashSet::new()),
        }
    }

    /// Creates a new `Indexer` without an index.
    ///
    /// # Type Parameters
    ///
    /// * `BE` - The backend type.
    ///
    /// # Arguments
    ///
    /// * `be` - The backend to write to.
    pub fn new_unindexed(be: BE) -> Self {
        Self {
            be,
            file: IndexFile::default(),
            count: 0,
            created: SystemTime::now(),
            indexed: None,
        }
    }

    /// Resets the indexer.
    pub fn reset(&mut self) {
        self.file = IndexFile::default();
        self.count = 0;
        self.created = SystemTime::now();
    }

    /// Returns a `SharedIndexer` to use in multiple threads.
    ///
    /// # Type Parameters
    ///
    /// * `BE` - The backend type.
    pub fn into_shared(self) -> SharedIndexer<BE> {
        Arc::new(RwLock::new(self))
    }

    /// Finalizes the `Indexer`.
    ///
    /// # Errors
    ///
    /// * [`CryptBackendErrorKind::SerializingToJsonByteVectorFailed`] - If the index file could not be serialized.
    ///
    /// [`CryptBackendErrorKind::SerializingToJsonByteVectorFailed`]: crate::error::CryptBackendErrorKind::SerializingToJsonByteVectorFailed
    pub fn finalize(&self) -> RusticResult<()> {
        self.save()
    }

    /// Save file if length of packs and `packs_to_delete` is greater than `0`.
    ///
    /// # Errors
    ///
    /// * [`CryptBackendErrorKind::SerializingToJsonByteVectorFailed`] - If the index file could not be serialized.
    ///
    /// [`CryptBackendErrorKind::SerializingToJsonByteVectorFailed`]: crate::error::CryptBackendErrorKind::SerializingToJsonByteVectorFailed
    pub fn save(&self) -> RusticResult<()> {
        if (self.file.packs.len() + self.file.packs_to_delete.len()) > 0 {
            _ = self.be.save_file(&self.file)?;
        }
        Ok(())
    }

    /// Adds a pack to the `Indexer`.
    ///
    /// # Arguments
    ///
    /// * `pack` - The pack to add.
    ///
    /// # Errors
    ///
    /// * [`IndexErrorKind::CouldNotGetElapsedTimeFromSystemTime`] - If the elapsed time could not be retrieved from the system time.
    /// * [`CryptBackendErrorKind::SerializingToJsonByteVectorFailed`] - If the index file could not be serialized.
    ///
    /// [`IndexErrorKind::CouldNotGetElapsedTimeFromSystemTime`]: crate::error::IndexErrorKind::CouldNotGetElapsedTimeFromSystemTime
    /// [`CryptBackendErrorKind::SerializingToJsonByteVectorFailed`]: crate::error::CryptBackendErrorKind::SerializingToJsonByteVectorFailed
    pub fn add(&mut self, pack: IndexPack) -> RusticResult<()> {
        self.add_with(pack, false)
    }

    /// Adds a pack to the `Indexer` and removes it from the backend.
    ///
    /// # Arguments
    ///
    /// * `pack` - The pack to add.
    ///
    /// # Errors
    ///
    /// * [`IndexErrorKind::CouldNotGetElapsedTimeFromSystemTime`] - If the elapsed time could not be retrieved from the system time.
    /// * [`CryptBackendErrorKind::SerializingToJsonByteVectorFailed`] - If the index file could not be serialized.
    ///
    /// [`IndexErrorKind::CouldNotGetElapsedTimeFromSystemTime`]: crate::error::IndexErrorKind::CouldNotGetElapsedTimeFromSystemTime
    /// [`CryptBackendErrorKind::SerializingToJsonByteVectorFailed`]: crate::error::CryptBackendErrorKind::SerializingToJsonByteVectorFailed
    pub fn add_remove(&mut self, pack: IndexPack) -> RusticResult<()> {
        self.add_with(pack, true)
    }

    /// Adds a pack to the `Indexer`.
    ///
    /// # Arguments
    ///
    /// * `pack` - The pack to add.
    /// * `delete` - Whether to delete the pack from the backend.
    ///
    /// # Errors
    ///
    /// * [`IndexErrorKind::CouldNotGetElapsedTimeFromSystemTime`] - If the elapsed time could not be retrieved from the system time.
    /// * [`CryptBackendErrorKind::SerializingToJsonByteVectorFailed`] - If the index file could not be serialized.
    ///
    /// [`IndexErrorKind::CouldNotGetElapsedTimeFromSystemTime`]: crate::error::IndexErrorKind::CouldNotGetElapsedTimeFromSystemTime
    /// [`CryptBackendErrorKind::SerializingToJsonByteVectorFailed`]: crate::error::CryptBackendErrorKind::SerializingToJsonByteVectorFailed
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

    /// Returns whether the given id is indexed.
    ///
    /// # Arguments
    ///
    /// * `id` - The id to check.
    pub fn has(&self, id: &Id) -> bool {
        self.indexed
            .as_ref()
            .map_or(false, |indexed| indexed.contains(id))
    }
}
