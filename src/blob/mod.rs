mod packer;
mod tree;
pub use crate::backend::node::*;
pub use packer::*;
pub use tree::*;

use derive_more::Constructor;
use serde::{Deserialize, Serialize};

use crate::id::Id;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum BlobType {
    #[serde(rename = "data")]
    Data,
    #[serde(rename = "tree")]
    Tree,
}

#[derive(Debug, PartialEq, Clone, Constructor)]
pub struct Blob {
    tpe: BlobType,
    id: Id,
}
