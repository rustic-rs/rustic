use std::{cmp::Ordering, num::NonZeroU32};

use chrono::{DateTime, Local};

use serde::{Deserialize, Serialize};

use crate::{
    backend::FileType, blob::BlobType, id::Id, repofile::packfile::PackHeaderRef,
    repofile::RepoFile,
};

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct IndexFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<Vec<Id>>,
    pub packs: Vec<IndexPack>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packs_to_delete: Vec<IndexPack>,
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

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct IndexPack {
    pub id: Id,
    pub blobs: Vec<IndexBlob>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<DateTime<Local>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
}

impl IndexPack {
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
    #[must_use]
    pub fn pack_size(&self) -> u32 {
        self.size
            .unwrap_or_else(|| PackHeaderRef::from_index_pack(self).pack_size())
    }

    /// returns the blob type of the pack. Note that only packs with
    /// identical blob types are allowed
    #[must_use]
    pub fn blob_type(&self) -> BlobType {
        // TODO: This is a hack to support packs without blobs (e.g. when deleting unreferenced files)
        if self.blobs.is_empty() {
            BlobType::Data
        } else {
            self.blobs[0].tpe
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Copy)]
pub struct IndexBlob {
    pub id: Id,
    #[serde(rename = "type")]
    pub tpe: BlobType,
    pub offset: u32,
    pub length: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uncompressed_length: Option<NonZeroU32>,
}

impl PartialOrd<Self> for IndexBlob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.offset.partial_cmp(&other.offset)
    }
}

impl Ord for IndexBlob {
    fn cmp(&self, other: &Self) -> Ordering {
        self.offset.cmp(&other.offset)
    }
}
