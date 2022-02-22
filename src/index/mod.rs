use crate::backend::{DecryptReadBackend, FileType};
use crate::blob::BlobType;
use crate::id::Id;
use anyhow::Result;
use derive_getters::{Dissolve, Getters};
use derive_more::Constructor;

mod boom;
mod indexer;
mod indexfiles;

pub use boom::*;
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
        Ok(be.read_partial(FileType::Pack, self.pack, self.offset, self.length)?)
    }
}

pub trait ReadIndex {
    fn get_id(&self, id: &Id) -> Option<IndexEntry>;

    fn has(&self, id: &Id) -> bool {
        self.get_id(id).is_some()
    }
}

pub trait IndexedEntry: ReadIndex + DecryptReadBackend {
    fn from_backend(&self, id: &Id) -> Option<Result<Vec<u8>>> {
        self.get_id(id).map(|ie| ie.read_data(self))
    }
}
