use crate::backend::{FileType, ReadBackend};
use crate::blob::{Blob, BlobType};
use crate::id::Id;
use crate::repo::IndexBlob;
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
    id: Id,
    offset: u32,
    length: u32,
}

impl IndexEntry {
    pub fn from_index_blob(pid: Id, ie: IndexBlob) -> Self {
        Self {
            pack: pid,
            tpe: *ie.tpe(),
            id: *ie.id(),
            offset: *ie.offset(),
            length: *ie.length(),
        }
    }

    /// Get a blob described by IndexEntry from the backend
    pub fn read_data<B: ReadBackend>(&self, be: &B) -> Result<Vec<u8>> {
        Ok(be.read_partial(FileType::Pack, self.pack, self.offset, self.length)?)
    }

    #[inline]
    pub fn blob(&self) -> Blob {
        Blob::new(self.tpe, self.id)
    }
}
pub trait ReadIndex {
    fn get_id(&self, id: &Id) -> Option<IndexEntry>;
}
