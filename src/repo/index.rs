use anyhow::Result;
use derive_getters::{Dissolve, Getters};
use serde::{Deserialize, Serialize};

use crate::backend::{FileType, ReadBackend};
use crate::blob::BlobType;
use crate::id::Id;

#[derive(Debug, Serialize, Deserialize, Getters, Dissolve)]
pub struct IndexFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    supersedes: Option<Vec<Id>>,
    packs: Vec<IndexPack>,
}

impl IndexFile {
    /// Get an IndexFile from the backend
    pub fn from_backend<B: ReadBackend>(be: &B, id: Id) -> Result<Self> {
        let data = be.read_full(FileType::Index, id)?;
        Ok(serde_json::from_slice(&data)?)
    }
}

#[derive(Debug, Serialize, Deserialize, Getters, Dissolve)]
pub struct IndexPack {
    id: Id,
    blobs: Vec<IndexBlob>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Getters, Dissolve)]
pub struct IndexBlob {
    id: Id,
    #[serde(rename = "type")]
    tpe: BlobType,
    offset: u32,
    length: u32,
}
