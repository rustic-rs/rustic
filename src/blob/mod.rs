mod packer;
mod tree;
pub use crate::backend::node::*;
pub use packer::*;
pub use tree::*;

use binrw::BinWrite;
use derive_more::Constructor;
use serde::{Deserialize, Serialize};

use crate::id::Id;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, BinWrite,
)]
pub enum BlobType {
    #[serde(rename = "tree")]
    #[bw(magic(1u8))]
    Tree,
    #[serde(rename = "data")]
    #[bw(magic(0u8))]
    Data,
}

impl BlobType {
    pub fn is_cacheable(&self) -> bool {
        match self {
            BlobType::Tree => true,
            BlobType::Data => false,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Constructor)]
pub struct Blob {
    tpe: BlobType,
    id: Id,
}
