use derive_getters::{Dissolve, Getters};
use serde::{Deserialize, Serialize};

use crate::backend::{FileType, RepoFile};
use crate::blob::BlobType;
use crate::id::Id;

#[derive(Debug, Serialize, Deserialize, Getters, Dissolve)]
pub struct IndexFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    supersedes: Option<Vec<Id>>,
    packs: Vec<IndexPack>,
}

impl RepoFile for IndexFile {
    const TYPE: FileType = FileType::Index;
}

impl IndexFile {
    pub fn new() -> Self {
        Self {
            supersedes: None,
            packs: Vec::new(),
        }
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

    // calculate the pack size from the contained blobs
    pub fn pack_size(&self) -> u32 {
        let mut size = 4 + 32; // 4 + crypto overhead
        for blob in &self.blobs {
            size += blob.length() + 37 // 37 = length of blob description
        }
        size
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
