use std::cmp::Ordering;
use std::num::NonZeroU32;

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::backend::{FileType, RepoFile};
use crate::blob::BlobType;
use crate::id::Id;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct IndexFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) supersedes: Option<Vec<Id>>,
    pub(crate) packs: Vec<IndexPack>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) packs_to_delete: Vec<IndexPack>,
}

impl RepoFile for IndexFile {
    const TYPE: FileType = FileType::Index;
}

impl IndexFile {
    pub fn add(&mut self, p: IndexPack, delete: bool) {
        if delete {
            self.packs_to_delete.push(p);
        } else {
            self.packs.push(p);
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct IndexPack {
    pub(crate) id: Id,
    pub(crate) blobs: Vec<IndexBlob>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) time: Option<DateTime<Local>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) size: Option<u32>,
}

impl IndexPack {
    // 4 equals the size of blob::packer::PackHeaderLength
    // 32 equals the size of the crypto overhead
    pub const PACK_OVERHEAD: u32 = 4 + 32;

    // this equals the size of blob::packer::PackHeaderEntry
    pub const HEADER_LEN: u32 = 37;
    // this equals the size of blob::packer::PackHeaderEntryComp
    pub const HEADER_LEN_COMPRESSED: u32 = 41;

    pub fn set_id(&mut self, id: Id) {
        self.id = id;
    }

    pub fn add(
        &mut self,
        id: Id,
        tpe: BlobType,
        offset: u32,
        length: u32,
        uncompressed_length: Option<NonZeroU32>,
    ) {
        self.blobs.push(IndexBlob {
            id,
            tpe,
            offset,
            length,
            uncompressed_length,
        });
    }

    // calculate the pack size from the contained blobs
    pub fn pack_size(&self) -> u32 {
        self.size.unwrap_or_else(|| {
            self.blobs.iter().fold(Self::PACK_OVERHEAD, |acc, blob| {
                acc + blob.length
                    + match blob.uncompressed_length {
                        None => Self::HEADER_LEN,
                        Some(_) => Self::HEADER_LEN_COMPRESSED,
                    }
            })
        })
    }

    /// returns the blob type of the pack. Note that only packs with
    /// identical blob types are allowed
    pub fn blob_type(&self) -> BlobType {
        // TODO: This is a hack to support packs without blobs (e.g. when deleting unreferenced files)
        if self.blobs.is_empty() {
            BlobType::Data
        } else {
            self.blobs[0].tpe
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct IndexBlob {
    pub(crate) id: Id,
    #[serde(rename = "type")]
    pub(crate) tpe: BlobType,
    pub(crate) offset: u32,
    pub(crate) length: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) uncompressed_length: Option<NonZeroU32>,
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
