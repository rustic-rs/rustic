mod packer;
mod tree;
pub use crate::backend::node::*;
pub use packer::*;
pub use tree::*;

use derive_more::Constructor;
use serde::{Deserialize, Serialize};

use crate::id::Id;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum BlobType {
    #[serde(rename = "tree")]
    Tree,
    #[serde(rename = "data")]
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
