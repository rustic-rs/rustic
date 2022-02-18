use anyhow::Result;
use derive_getters::{Dissolve, Getters};
use serde::{Deserialize, Serialize};

use crate::backend::{FileType, ReadBackend, WriteBackend};
use crate::blob::BlobType;
use crate::id::Id;

#[derive(Debug, Serialize, Deserialize, Getters, Dissolve)]
pub struct IndexFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    supersedes: Option<Vec<Id>>,
    packs: Vec<IndexPack>,
}

impl IndexFile {
    pub fn new() -> Self {
        Self {
            supersedes: None,
            packs: Vec::new(),
        }
    }

    /// Get an IndexFile from the backend
    pub fn from_backend<B: ReadBackend>(be: &B, id: Id) -> Result<Self> {
        let data = be.read_full(FileType::Index, id)?;
        Ok(serde_json::from_slice(&data)?)
    }

    /// Sace an IndexFile to the backend
    pub fn save_to_backend<B: WriteBackend>(&self, be: &B) -> Result<()> {
        let data = serde_json::to_vec(&self)?;
        be.hash_write_full(FileType::Index, &data)?;
        Ok(())
    }

    pub fn add(&mut self, p: IndexPack) {
        self.packs.push(p);
    }
}

#[derive(Debug, Serialize, Deserialize, Getters, Dissolve)]
pub struct IndexPack {
    id: Id,
    blobs: Vec<IndexBlob>,
}

impl IndexPack {
    pub fn new() -> Self {
        Self {
            id: Id::default(),
            blobs: Vec::new(),
        }
    }

    pub fn set_id(&mut self, id: Id) {
        self.id = id;
    }

    pub fn add(&mut self, id: Id, tpe: BlobType, offset: u32, length: u32) {
        self.blobs.push(IndexBlob {
            id,
            tpe,
            offset,
            length,
        });
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Getters, Dissolve)]
pub struct IndexBlob {
    id: Id,
    #[serde(rename = "type")]
    tpe: BlobType,
    offset: u32,
    length: u32,
}
