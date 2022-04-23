use std::sync::Arc;

use ambassador::{delegatable_trait, Delegate};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use derive_getters::Getters;
use derive_more::Constructor;
use futures::StreamExt;
use indicatif::ProgressBar;
use vlog::*;

use crate::backend::{DecryptReadBackend, FileType};
use crate::blob::BlobType;
use crate::id::Id;
use crate::repo::IndexFile;

mod binarysorted;
mod indexer;

use binarysorted::BinarySortedIndex;
pub use indexer::*;

#[derive(Debug, Clone, Constructor, Getters)]
pub struct IndexEntry {
    pack: Id,
    offset: u32,
    length: u32,
}

impl IndexEntry {
    /// Get a blob described by IndexEntry from the backend
    pub async fn read_data<B: DecryptReadBackend>(&self, be: &B) -> Result<Vec<u8>> {
        be.read_encrypted_partial(FileType::Pack, &self.pack, self.offset, self.length)
            .await
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
    index: Arc<BinarySortedIndex>,
}

impl<BE: DecryptReadBackend> IndexBackend<BE> {
    pub async fn new(be: &BE, p: ProgressBar) -> Result<Self> {
        v1!("reading index...");
        let index = BinarySortedIndex::full(
            be.stream_all::<IndexFile>(p.clone())
                .await?
                .map(|i| i.unwrap().1),
        )
        .await;
        p.finish_with_message("done");
        Ok(Self {
            be: be.clone(),
            index: Arc::new(index),
        })
    }

    pub async fn only_full_trees(be: &BE, p: ProgressBar) -> Result<Self> {
        v1!("reading index...");
        let index = BinarySortedIndex::only_full_trees(
            be.stream_all::<IndexFile>(p.clone())
                .await?
                .map(|i| i.unwrap().1),
        )
        .await;
        p.finish_with_message("done");
        Ok(Self {
            be: be.clone(),
            index: Arc::new(index),
        })
    }
}

impl<BE: DecryptReadBackend> IndexedBackend for IndexBackend<BE> {
    type Backend = BE;

    fn be(&self) -> &Self::Backend {
        &self.be
    }
}
