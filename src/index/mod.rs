use std::num::NonZeroU32;
use std::sync::Arc;

use ambassador::{delegatable_trait, Delegate};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use derive_getters::Getters;
use derive_more::Constructor;
use futures::StreamExt;
use indicatif::ProgressBar;
use vlog::*;
use zstd::decode_all;

use crate::backend::{DecryptReadBackend, FileType};
use crate::blob::BlobType;
use crate::id::Id;
use crate::repo::{IndexBlob, IndexFile};

mod binarysorted;
mod indexer;

pub use binarysorted::*;
pub use indexer::*;

#[derive(Debug, Clone, Constructor, Getters)]
pub struct IndexEntry {
    pack: Id,
    offset: u32,
    length: u32,
    uncompressed_length: Option<NonZeroU32>,
}

impl IndexEntry {
    pub fn from_index_blob(blob: &IndexBlob, pack: Id) -> Self {
        Self {
            pack,
            offset: blob.offset,
            length: blob.length,
            uncompressed_length: blob.uncompressed_length,
        }
    }

    /// Get a blob described by IndexEntry from the backend
    pub async fn read_data<B: DecryptReadBackend>(&self, be: &B) -> Result<Vec<u8>> {
        let data = be
            .read_encrypted_partial(FileType::Pack, &self.pack, self.offset, self.length)
            .await?;
        Ok(match self.uncompressed_length {
            None => data,
            Some(_) => decode_all(&*data)?,
        })
    }

    pub fn data_length(&self) -> u32 {
        match self.uncompressed_length {
            None => self.length - 32, // crypto overhead
            Some(length) => length.get(),
        }
    }
}

#[delegatable_trait]
pub trait ReadIndex {
    fn get_id(&self, tpe: &BlobType, id: &Id) -> Option<IndexEntry>;

    fn get_tree(&self, id: &Id) -> Option<IndexEntry> {
        self.get_id(&BlobType::Tree, id)
    }

    fn get_data(&self, id: &Id) -> Option<IndexEntry> {
        self.get_id(&BlobType::Data, id)
    }

    fn has(&self, tpe: &BlobType, id: &Id) -> bool {
        self.get_id(tpe, id).is_some()
    }

    fn has_tree(&self, id: &Id) -> bool {
        self.has(&BlobType::Tree, id)
    }

    fn has_data(&self, id: &Id) -> bool {
        self.has(&BlobType::Data, id)
    }
}

#[async_trait]
pub trait IndexedBackend: ReadIndex + Clone + Sync + Send + 'static {
    type Backend: DecryptReadBackend;

    fn be(&self) -> &Self::Backend;

    async fn blob_from_backend(&self, tpe: &BlobType, id: &Id) -> Result<Vec<u8>> {
        match self.get_id(tpe, id) {
            None => Err(anyhow!("blob not found in index")),
            Some(ie) => ie.read_data(self.be()).await,
        }
    }
}

#[derive(Clone, Delegate)]
#[delegate(ReadIndex, target = "index")]
pub struct IndexBackend<BE: DecryptReadBackend> {
    be: BE,
    index: Arc<Index>,
}

impl<BE: DecryptReadBackend> IndexBackend<BE> {
    pub fn new_from_index(be: &BE, index: Index) -> Self {
        Self {
            be: be.clone(),
            index: Arc::new(index),
        }
    }

    async fn new_from_collector(
        be: &BE,
        p: ProgressBar,
        mut collector: IndexCollector,
    ) -> Result<Self> {
        v1!("reading index...");
        let mut stream = be
            .stream_all::<IndexFile>(p.clone())
            .await?
            .map(|i| i.unwrap().1);

        while let Some(index) = stream.next().await {
            collector.extend(index.packs);
        }
        p.finish();

        Ok(Self::new_from_index(be, collector.into_index()))
    }

    pub async fn new(be: &BE, p: ProgressBar) -> Result<Self> {
        Self::new_from_collector(be, p, IndexCollector::new(IndexType::Full)).await
    }

    pub async fn only_full_trees(be: &BE, p: ProgressBar) -> Result<Self> {
        Self::new_from_collector(be, p, IndexCollector::new(IndexType::FullTrees)).await
    }
}

impl<BE: DecryptReadBackend> IndexedBackend for IndexBackend<BE> {
    type Backend = BE;

    fn be(&self) -> &Self::Backend {
        &self.be
    }
}
