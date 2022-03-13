use std::rc::Rc;

use ambassador::{delegatable_trait, Delegate};
use anyhow::{anyhow, Result};
use derive_getters::{Dissolve, Getters};
use derive_more::Constructor;
use indicatif::ProgressBar;
use vlog::*;

use crate::backend::{DecryptReadBackend, FileType};
use crate::blob::BlobType;
use crate::id::Id;

mod boom;
mod indexer;
mod indexfiles;

use boom::BoomIndex;
pub use indexer::*;
pub use indexfiles::*;

#[derive(Debug, Clone, Constructor, Getters, Dissolve)]
pub struct IndexEntry {
    pack: Id,
    offset: u32,
    length: u32,
}

impl IndexEntry {
    /// Get a blob described by IndexEntry from the backend
    pub fn read_data<B: DecryptReadBackend>(&self, be: &B) -> Result<Vec<u8>> {
        Ok(be.read_encrypted_partial(FileType::Pack, &self.pack, self.offset, self.length)?)
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

pub trait IndexedBackend: Clone + ReadIndex {
    type Backend: DecryptReadBackend;

    fn be(&self) -> &Self::Backend;

    fn blob_from_backend(&self, tpe: &BlobType, id: &Id) -> Result<Vec<u8>> {
        self.get_id(tpe, id)
            .map(|ie| ie.read_data(self.be()))
            .ok_or(anyhow!("blob not found in index"))?
    }
}

#[derive(Clone, Delegate)]
#[delegate(ReadIndex, target = "index")]
pub struct IndexBackend<BE: DecryptReadBackend> {
    be: BE,
    index: Rc<BoomIndex>,
}

impl<BE: DecryptReadBackend> IndexBackend<BE> {
    pub fn new(be: &BE, p: ProgressBar) -> Result<Self> {
        v1!("reading index...");
        let index = Rc::new(
            AllIndexFiles::new(be.clone())
                .into_iter(p.clone())?
                .collect(),
        );
        p.finish_with_message("done");
        Ok(Self {
            be: be.clone(),
            index,
        })
    }

    pub fn only_full_trees(be: &BE, p: ProgressBar) -> Result<Self> {
        v1!("reading index...");
        let index = Rc::new(BoomIndex::only_full_trees(
            AllIndexFiles::new(be.clone()).into_iter(p.clone())?,
        ));
        p.finish_with_message("done");
        Ok(Self {
            be: be.clone(),
            index,
        })
    }
}

impl<BE: DecryptReadBackend> IndexedBackend for IndexBackend<BE> {
    type Backend = BE;

    fn be(&self) -> &Self::Backend {
        &self.be
    }
}
