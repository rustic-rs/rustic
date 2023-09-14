use std::{num::NonZeroU32, sync::Arc, thread::sleep, time::Duration};

use bytes::Bytes;
use derive_more::Constructor;

use crate::{
    backend::{decrypt::DecryptReadBackend, FileType},
    blob::BlobType,
    error::{IndexErrorKind, RusticResult},
    id::Id,
    index::binarysorted::{Index, IndexCollector, IndexType},
    progress::Progress,
    repofile::indexfile::{IndexBlob, IndexFile},
};

pub(crate) mod binarysorted;
pub(crate) mod indexer;

/// An entry in the index
#[derive(Debug, Clone, Copy, PartialEq, Eq, Constructor)]
pub struct IndexEntry {
    /// The type of the blob
    blob_type: BlobType,
    /// The pack the blob is in
    pub pack: Id,
    /// The offset of the blob in the pack
    pub offset: u32,
    /// The length of the blob in the pack
    pub length: u32,
    /// The uncompressed length of the blob
    pub uncompressed_length: Option<NonZeroU32>,
}

impl IndexEntry {
    /// Create an [`IndexEntry`] from an [`IndexBlob`]
    ///
    /// # Arguments
    ///
    /// * `blob` - The [`IndexBlob`] to create the [`IndexEntry`] from
    /// * `pack` - The pack the blob is in
    #[must_use]
    pub const fn from_index_blob(blob: &IndexBlob, pack: Id) -> Self {
        Self {
            blob_type: blob.tpe,
            pack,
            offset: blob.offset,
            length: blob.length,
            uncompressed_length: blob.uncompressed_length,
        }
    }

    /// Get a blob described by [`IndexEntry`] from the backend
    ///
    /// # Arguments
    ///
    /// * `be` - The backend to read from
    ///
    /// # Errors
    ///
    // TODO:  add error! This function will return an error if the blob is not found in the backend.
    pub fn read_data<B: DecryptReadBackend>(&self, be: &B) -> RusticResult<Bytes> {
        let data = be.read_encrypted_partial(
            FileType::Pack,
            &self.pack,
            self.blob_type.is_cacheable(),
            self.offset,
            self.length,
            self.uncompressed_length,
        )?;
        Ok(data)
    }

    /// Get the length of the data described by the [`IndexEntry`]
    #[must_use]
    pub const fn data_length(&self) -> u32 {
        match self.uncompressed_length {
            None => self.length - 32, // crypto overhead
            Some(length) => length.get(),
        }
    }
}

/// The index of the repository
///
/// The index is a list of [`IndexEntry`]s
pub trait ReadIndex {
    /// Get an [`IndexEntry`] from the index
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the blob
    /// * `id` - The id of the blob
    ///
    /// # Returns
    ///
    /// The [`IndexEntry`] - If it exists otherwise `None`
    fn get_id(&self, tpe: BlobType, id: &Id) -> Option<IndexEntry>;

    /// Get the total size of all blobs of the given type
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the blobs
    fn total_size(&self, tpe: BlobType) -> u64;

    /// Check if the index contains the given blob
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the blob
    /// * `id` - The id of the blob
    fn has(&self, tpe: BlobType, id: &Id) -> bool;

    /// Get a tree from the index
    ///
    /// # Arguments
    ///
    /// * `id` - The id of the tree
    ///
    /// # Returns
    ///
    /// The [`IndexEntry`] of the tree if it exists otherwise `None`
    fn get_tree(&self, id: &Id) -> Option<IndexEntry> {
        self.get_id(BlobType::Tree, id)
    }

    /// Get a data blob from the index
    ///
    /// # Arguments
    ///
    /// * `id` - The id of the data blob
    ///
    /// # Returns
    ///
    /// The [`IndexEntry`] of the data blob if it exists otherwise `None`
    fn get_data(&self, id: &Id) -> Option<IndexEntry> {
        self.get_id(BlobType::Data, id)
    }

    /// Check if the index contains the given tree
    ///
    /// # Arguments
    ///
    /// * `id` - The id of the tree
    ///
    /// # Returns
    ///
    /// `true` if the index contains the tree otherwise `false`
    fn has_tree(&self, id: &Id) -> bool {
        self.has(BlobType::Tree, id)
    }

    /// Check if the index contains the given data blob
    ///
    /// # Arguments
    ///
    /// * `id` - The id of the data blob
    ///
    /// # Returns
    ///
    /// `true` if the index contains the data blob otherwise `false`
    fn has_data(&self, id: &Id) -> bool {
        self.has(BlobType::Data, id)
    }
}

/// A trait for backends with an index
pub trait IndexedBackend: ReadIndex + Clone + Sync + Send + 'static {
    /// The backend type
    type Backend: DecryptReadBackend;

    /// Get a reference to the backend
    fn be(&self) -> &Self::Backend;

    /// Get a blob from the backend
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the blob
    /// * `id` - The id of the blob
    ///
    /// # Errors
    ///
    /// If the blob could not be found in the backend
    ///
    /// # Returns
    ///
    /// The data of the blob
    fn blob_from_backend(&self, tpe: BlobType, id: &Id) -> RusticResult<Bytes>;
}

/// A backend with an index
///
/// # Type Parameters
///
/// * `BE` - The backend type
#[derive(Clone, Debug)]
pub struct IndexBackend<BE: DecryptReadBackend> {
    /// The backend to read from.
    be: BE,
    /// The atomic reference counted, sharable index.
    index: Arc<Index>,
}

impl<BE: DecryptReadBackend> ReadIndex for IndexBackend<BE> {
    /// Get an [`IndexEntry`] from the index
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the blob
    /// * `id` - The id of the blob
    ///
    /// # Returns
    ///
    /// The [`IndexEntry`] - If it exists otherwise `None`
    fn get_id(&self, tpe: BlobType, id: &Id) -> Option<IndexEntry> {
        self.index.get_id(tpe, id)
    }

    /// Get the total size of all blobs of the given type
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the blobs
    fn total_size(&self, tpe: BlobType) -> u64 {
        self.index.total_size(tpe)
    }

    /// Check if the index contains the given blob
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the blob
    /// * `id` - The id of the blob
    ///
    /// # Returns
    ///
    /// `true` if the index contains the blob otherwise `false`
    fn has(&self, tpe: BlobType, id: &Id) -> bool {
        self.index.has(tpe, id)
    }
}

impl<BE: DecryptReadBackend> IndexBackend<BE> {
    /// Create a new [`IndexBackend`] from an [`Index`]
    ///
    /// # Type Parameters
    ///
    /// * `BE` - The backend type
    ///
    /// # Arguments
    ///
    /// * `be` - The backend to read from
    /// * `index` - The index to use
    pub fn new_from_index(be: &BE, index: Index) -> Self {
        Self {
            be: be.clone(),
            index: Arc::new(index),
        }
    }

    /// Create a new [`IndexBackend`] from an [`IndexCollector`]
    ///
    /// # Type Parameters
    ///
    /// * `BE` - The backend type
    ///
    /// # Arguments
    ///
    /// * `be` - The backend to read from
    /// * `p` - The progress tracker
    /// * `collector` - The [`IndexCollector`] to use
    ///
    /// # Errors
    ///
    /// If the index could not be read
    fn new_from_collector(
        be: &BE,
        p: &impl Progress,
        mut collector: IndexCollector,
    ) -> RusticResult<Self> {
        p.set_title("reading index...");
        for index in be.stream_all::<IndexFile>(p)? {
            collector.extend(index?.1.packs);
        }

        p.finish();

        Ok(Self::new_from_index(be, collector.into_index()))
    }

    /// Create a new [`IndexBackend`]
    ///
    /// # Type Parameters
    ///
    /// * `BE` - The backend type
    ///
    /// # Arguments
    ///
    /// * `be` - The backend to read from
    /// * `p` - The progress tracker
    pub fn new(be: &BE, p: &impl Progress) -> RusticResult<Self> {
        Self::new_from_collector(be, p, IndexCollector::new(IndexType::Full))
    }

    /// Create a new [`IndexBackend`] with only full trees
    ///
    /// # Type Parameters
    ///
    /// * `BE` - The backend type
    ///
    /// # Arguments
    ///
    /// * `be` - The backend to read from
    /// * `p` - The progress tracker
    ///
    /// # Errors
    ///
    /// If the index could not be read
    pub fn only_full_trees(be: &BE, p: &impl Progress) -> RusticResult<Self> {
        Self::new_from_collector(be, p, IndexCollector::new(IndexType::DataIds))
    }

    /// Convert the `Arc<Index>` to an Index
    pub fn into_index(self) -> Index {
        match Arc::try_unwrap(self.index) {
            Ok(index) => index,
            Err(arc) => {
                // Seems index is still in use; this could be due to some threads using it which didn't yet completely shut down.
                // sleep a bit to let threads using the index shut down, after this index should be available to unwrap
                sleep(Duration::from_millis(100));
                Arc::try_unwrap(arc).expect("index still in use")
            }
        }
    }
}

impl<BE: DecryptReadBackend> IndexedBackend for IndexBackend<BE> {
    type Backend = BE;

    /// Get a reference to the backend
    fn be(&self) -> &Self::Backend {
        &self.be
    }

    /// Get a blob from the backend
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the blob
    /// * `id` - The id of the blob
    ///
    /// # Errors
    ///
    /// * [`IndexErrorKind::BlobInIndexNotFound`] - If the blob could not be found in the index
    ///
    /// [`IndexErrorKind::BlobInIndexNotFound`]: crate::error::IndexErrorKind::BlobInIndexNotFound
    fn blob_from_backend(&self, tpe: BlobType, id: &Id) -> RusticResult<Bytes> {
        self.get_id(tpe, id).map_or_else(
            || Err(IndexErrorKind::BlobInIndexNotFound.into()),
            |ie| ie.read_data(self.be()),
        )
    }
}
