use crate::backend::{FileType, ReadBackend};
use crate::blob::BlobType;
use crate::id::Id;
use anyhow::Result;
use derive_getters::{Dissolve, Getters};
use derive_more::Constructor;

pub mod boom;
pub mod indexfiles;

pub use boom::*;
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
    pub fn read_data<B: ReadBackend>(&self, be: &B) -> Result<Vec<u8>> {
        Ok(be.read_partial(FileType::Pack, self.pack, self.offset, self.length)?)
    }
}
pub trait ReadIndex {
    fn get_id(&self, id: &Id) -> Option<IndexEntry>;
}
