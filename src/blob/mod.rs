use anyhow::Result;
use derive_new::new;
use serde::{Deserialize, Serialize};

use crate::backend::{FileType, ReadBackend};
use crate::id::Id;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum BlobType {
    #[serde(rename = "data")]
    Data,
    #[serde(rename = "tree")]
    Tree,
}

#[derive(Debug, PartialEq, new)]
pub struct Blob {
    tpe: BlobType,
    id: Id,
}

#[derive(Debug, new)]
pub struct BlobInformation {
    blob: Blob,
    offset: u32,
    length: u32,
}

#[derive(Debug, new)]
pub struct IndexEntry {
    pack: Id,
    bi: BlobInformation,
}

impl IndexEntry {
    /// Get a blob described by IndexEntry from the backend
    pub fn read_data<B: ReadBackend>(&self, be: &B) -> Result<Vec<u8>> {
        Ok(be.read_partial(FileType::Pack, self.pack, self.offset(), self.length())?)
    }

    #[inline]
    pub fn id(&self) -> &Id {
        &self.bi.blob.id
    }

    #[inline]
    pub fn tpe(&self) -> &BlobType {
        &self.bi.blob.tpe
    }

    #[inline]
    pub fn offset(&self) -> u32 {
        self.bi.offset
    }

    #[inline]
    pub fn length(&self) -> u32 {
        self.bi.length
    }

    #[inline]
    pub fn blob(&self) -> &Blob {
        &self.bi.blob
    }
}
