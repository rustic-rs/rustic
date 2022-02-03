use serde::{Deserialize, Serialize};

use crate::id::Id;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum BlobType {
    #[serde(rename = "data")]
    Data,
    #[serde(rename = "tree")]
    Tree,
}

#[derive(Debug, PartialEq)]
pub struct Blob {
    pub tpe: BlobType,
    pub id: Id,
}

#[derive(Debug)]
pub struct BlobInformation {
    pub blob: Blob,
    pub offset: u32,
    pub length: u32,
}

#[derive(Debug)]
pub struct IndexEntry {
    pub pack: Id,
    pub bi: BlobInformation,
}
