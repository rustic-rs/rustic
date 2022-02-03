use anyhow::Result;
use std::iter::Iterator;

use serde::{Deserialize, Serialize};

use crate::backend::{FileType, ReadBackend};
use crate::blob::{Blob, BlobInformation, BlobType, IndexEntry};
use crate::id::Id;
use crate::index::ReadIndex;

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
}

#[derive(Debug, Serialize, Deserialize)]
struct PackIndex {
    id: Id,
    blobs: Vec<BlobIndex>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BlobIndex {
    id: Id,
    #[serde(rename = "type")]
    tpe: BlobType,
    offset: u32,
    length: u32,
}

impl BlobIndex {
    pub fn to_bi(&self) -> BlobInformation {
        BlobInformation {
            blob: Blob {
                id: self.id,
                tpe: self.tpe,
            },
            offset: self.offset,
            length: self.length,
        }
    }
}

impl ReadIndex for IndexFile {
    fn iter(&self) -> Box<dyn Iterator<Item = IndexEntry> + '_> {
        Box::new(self.packs.iter().flat_map(|p| {
            p.blobs.iter().map(|b| IndexEntry {
                pack: p.id,
                bi: b.to_bi(),
            })
        }))
    }
}
