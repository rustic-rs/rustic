use anyhow::Result;

use serde::{Deserialize, Serialize};

use crate::backend::{FileType, ReadBackend};
use crate::blob::{Blob, BlobInformation, BlobType};
use crate::id::Id;

#[derive(Debug, Serialize, Deserialize)]
pub struct IndexFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    supersedes: Option<Id>,
    packs: Vec<PackIndex>,
}

impl IndexFile {
    /// Get an IndexFile from the backend
    pub fn from_backend<B: ReadBackend>(be: &B, id: Id) -> Result<Self> {
        let data = be.read_full(FileType::Index, id)?;
        Ok(serde_json::from_slice(&data)?)
    }

    /// Packs contained in the IndexFile
    pub fn packs(self) -> Vec<PackIndex> {
        self.packs
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackIndex {
    id: Id,
    blobs: Vec<BlobIndex>,
}

impl PackIndex {
    pub fn id(&self) -> &Id {
        &self.id
    }
    pub fn blobs(self) -> Vec<BlobIndex> {
        self.blobs
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BlobIndex {
    id: Id,
    #[serde(rename = "type")]
    tpe: BlobType,
    offset: u32,
    length: u32,
}

impl BlobIndex {
    pub fn to_bi(&self) -> BlobInformation {
        BlobInformation::new(Blob::new(self.tpe, self.id), self.offset, self.length)
    }
}
