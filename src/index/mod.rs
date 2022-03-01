use std::rc::Rc;

use ambassador::{delegatable_trait, Delegate};
use anyhow::Result;
use derive_getters::{Dissolve, Getters};
use derive_more::Constructor;

use crate::backend::{DecryptReadBackend, FileType};
use crate::blob::BlobType;
use crate::id::Id;

#[cfg(feature = "boomphf")]
mod boom;
#[cfg(not(feature = "boomphf"))]
mod hashmap;
mod indexer;
mod indexfiles;

#[cfg(feature = "boomphf")]
use boom::BoomIndex;
#[cfg(not(feature = "boomphf"))]
use hashmap::HashMapIndex;
pub use indexer::*;
pub use indexfiles::*;

#[derive(Debug, Clone, Constructor, Getters, Dissolve)]
pub struct IndexEntry {
    pack: Id,
    tpe: BlobType,
    offset: u32,
    length: u32,
}

impl IndexEntry {
    /// Get a blob described by IndexEntry from the backend
    pub fn read_data<B: DecryptReadBackend>(&self, be: &B) -> Result<Vec<u8>> {
        Ok(be.read_partial(FileType::Pack, &self.pack, self.offset, self.length)?)
    }
}

#[delegatable_trait]
pub trait ReadIndex {
    fn get_id(&self, id: &Id) -> Option<IndexEntry>;

    fn has(&self, id: &Id) -> bool {
        self.get_id(id).is_some()
    }
}

pub trait IndexedBackend: Clone + ReadIndex {
    type Backend: DecryptReadBackend;

    fn be(&self) -> &Self::Backend;

    fn blob_from_backend(&self, id: &Id) -> Option<Result<Vec<u8>>> {
        self.get_id(id).map(|ie| ie.read_data(self.be()))
    }
}

#[derive(Clone, Delegate)]
#[delegate(ReadIndex, target = "index")]
pub struct IndexBackend<BE: DecryptReadBackend> {
    be: BE,
    #[cfg(feature = "boomphf")]
    index: Rc<BoomIndex>,
    #[cfg(not(feature = "boomphf"))]
    index: Rc<HashMapIndex>,
}

impl<BE: DecryptReadBackend> IndexBackend<BE> {
    pub fn new(be: &BE) -> Result<Self> {
        Ok(Self {
            be: be.clone(),
            index: Rc::new(AllIndexFiles::new(be.clone()).into_iter()?.collect()),
        })
    }
}

impl<BE: DecryptReadBackend> IndexedBackend for IndexBackend<BE> {
    type Backend = BE;

    fn be(&self) -> &Self::Backend {
        &self.be
    }
}
