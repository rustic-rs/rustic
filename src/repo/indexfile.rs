use std::cmp::Ordering;

use derive_getters::{Dissolve, Getters};
use serde::{Deserialize, Serialize};

use crate::backend::{FileType, RepoFile};
use crate::blob::BlobType;
use crate::id::Id;

#[derive(Debug, Default, Serialize, Deserialize, Getters, Dissolve)]
pub struct IndexFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    supersedes: Option<Vec<Id>>,
    packs: Vec<IndexPack>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    packs_to_delete: Vec<IndexPack>,
}

impl RepoFile for IndexFile {
    const TYPE: FileType = FileType::Index;
}

impl IndexFile {
    pub fn new() -> Self {
        Self {
            supersedes: None,
            packs: Vec::new(),
            packs_to_delete: Vec::new(),
        }
    }

    pub fn add(&mut self, p: IndexPack) {
        self.packs.push(p);
    }

    pub fn len(&self) -> usize {
        self.packs.iter().map(|p| p.blobs.len()).sum()
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

    /// returns the blob type of the pack. Note that only packs with
    /// identical blob types are allowed
    pub fn blob_type(&self) -> BlobType {
        self.blobs[0].tpe
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Getters, Dissolve, Eq, PartialEq)]
pub struct IndexBlob {
    id: Id,
    #[serde(rename = "type")]
    tpe: BlobType,
    offset: u32,
    length: u32,
}

impl PartialOrd<IndexBlob> for IndexBlob {
    fn partial_cmp(&self, other: &IndexBlob) -> Option<Ordering> {
        self.offset.partial_cmp(&other.offset)
    }
}

impl Ord for IndexBlob {
    fn cmp(&self, other: &Self) -> Ordering {
        self.offset.cmp(&other.offset)
    }
}
