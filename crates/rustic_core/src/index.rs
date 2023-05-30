use std::{num::NonZeroU32, sync::Arc, thread::sleep, time::Duration};

use bytes::Bytes;
use derive_more::Constructor;
use indicatif::ProgressBar;

use crate::{
    backend::{decrypt::DecryptReadBackend, FileType},
    blob::BlobType,
    error::{IndexErrorKind, RusticResult},
    id::Id,
    index::binarysorted::{Index, IndexCollector, IndexType},
    repofile::indexfile::{IndexBlob, IndexFile},
};

pub(crate) mod binarysorted;
pub(crate) mod indexer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Constructor)]
pub struct IndexEntry {
    blob_type: BlobType,
    pub pack: Id,
    pub offset: u32,
    pub length: u32,
    pub uncompressed_length: Option<NonZeroU32>,
}

impl IndexEntry {
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
    /// # Errors
    ///
    /// TODO This function will return an error if  .
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

    #[must_use]
    pub const fn data_length(&self) -> u32 {
        match self.uncompressed_length {
            None => self.length - 32, // crypto overhead
            Some(length) => length.get(),
        }
    }
}

pub trait ReadIndex {
    fn get_id(&self, tpe: BlobType, id: &Id) -> Option<IndexEntry>;
    fn total_size(&self, tpe: BlobType) -> u64;
    fn has(&self, tpe: BlobType, id: &Id) -> bool;

    fn get_tree(&self, id: &Id) -> Option<IndexEntry> {
        self.get_id(BlobType::Tree, id)
    }

    fn get_data(&self, id: &Id) -> Option<IndexEntry> {
        self.get_id(BlobType::Data, id)
    }

    fn has_tree(&self, id: &Id) -> bool {
        self.has(BlobType::Tree, id)
    }

    fn has_data(&self, id: &Id) -> bool {
        self.has(BlobType::Data, id)
    }
}

pub trait IndexedBackend: ReadIndex + Clone + Sync + Send + 'static {
    type Backend: DecryptReadBackend;

    fn be(&self) -> &Self::Backend;

    fn blob_from_backend(&self, tpe: BlobType, id: &Id) -> RusticResult<Bytes>;
}

#[derive(Clone, Debug)]
pub struct IndexBackend<BE: DecryptReadBackend> {
    be: BE,
    index: Arc<Index>,
}

impl<BE: DecryptReadBackend> ReadIndex for IndexBackend<BE> {
    fn get_id(&self, tpe: BlobType, id: &Id) -> Option<IndexEntry> {
        self.index.get_id(tpe, id)
    }

    fn total_size(&self, tpe: BlobType) -> u64 {
        self.index.total_size(tpe)
    }
    fn has(&self, tpe: BlobType, id: &Id) -> bool {
        self.index.has(tpe, id)
    }
}

impl<BE: DecryptReadBackend> IndexBackend<BE> {
    pub fn new_from_index(be: &BE, index: Index) -> Self {
        Self {
            be: be.clone(),
            index: Arc::new(index),
        }
    }

    fn new_from_collector(
        be: &BE,
        p: &ProgressBar,
        mut collector: IndexCollector,
    ) -> RusticResult<Self> {
        p.set_prefix("reading index...");
        for index in be.stream_all::<IndexFile>(p.clone())? {
            collector.extend(index?.1.packs);
        }

        p.finish();

        Ok(Self::new_from_index(be, collector.into_index()))
    }

    pub fn new(be: &BE, p: ProgressBar) -> RusticResult<Self> {
        Self::new_from_collector(be, &p, IndexCollector::new(IndexType::Full))
    }

    pub fn only_full_trees(be: &BE, p: ProgressBar) -> RusticResult<Self> {
        Self::new_from_collector(be, &p, IndexCollector::new(IndexType::FullTrees))
    }

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

    fn be(&self) -> &Self::Backend {
        &self.be
    }

    fn blob_from_backend(&self, tpe: BlobType, id: &Id) -> RusticResult<Bytes> {
        self.get_id(tpe, id).map_or_else(
            || Err(IndexErrorKind::BlobInIndexNotFound.into()),
            |ie| ie.read_data(self.be()),
        )
    }
}
