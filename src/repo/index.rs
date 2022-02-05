use anyhow::Result;

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

impl IntoIterator for IndexFile {
    type Item = IndexEntry;
    type IntoIter = Box<dyn Iterator<Item = IndexEntry>>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.packs.into_iter().flat_map(|p| {
            p.blobs
                .into_iter()
                .map(move |b| IndexEntry::new(p.id, b.to_bi()))
        }))
    }
}

impl ReadIndex for IndexFile {
    fn iter(&self) -> Box<dyn Iterator<Item = IndexEntry> + '_> {
        Box::new(
            self.packs
                .iter()
                .flat_map(|p| p.blobs.iter().map(|b| IndexEntry::new(p.id, b.to_bi()))),
        )
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
        BlobInformation::new(Blob::new(self.tpe, self.id), self.offset, self.length)
    }
}
