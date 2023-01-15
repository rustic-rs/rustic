mod packer;
mod tree;
use std::ops::Add;

pub use crate::backend::node::*;
pub use packer::*;
pub use tree::*;

use derive_more::Constructor;
use enum_map::{Enum, EnumMap};
use serde::{Deserialize, Serialize};

use crate::id::Id;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Enum,
)]
pub enum BlobType {
    #[serde(rename = "tree")]
    Tree,
    #[serde(rename = "data")]
    Data,
}

impl BlobType {
    pub fn is_cacheable(self) -> bool {
        match self {
            BlobType::Tree => true,
            BlobType::Data => false,
        }
    }
}

pub type BlobTypeMap<T> = EnumMap<BlobType, T>;

/// Initialize is a new trait to define the method init() for a [`BlobTypeMap`]
pub trait Initialize<T: Default + Sized> {
    /// initialize a [`BlobTypeMap`] by processing a given function for each [`BlobType`]
    fn init<F: FnMut(BlobType) -> T>(init: F) -> BlobTypeMap<T>;
}

impl<T: Default> Initialize<T> for BlobTypeMap<T> {
    fn init<F: FnMut(BlobType) -> T>(mut init: F) -> Self {
        let mut btm = BlobTypeMap::default();
        for i in 0..BlobType::LENGTH {
            let bt = BlobType::from_usize(i);
            btm[bt] = init(bt);
        }
        btm
    }
}

/// Sum is a new trait to define the method sum() for a [`BlobTypeMap`]
pub trait Sum<T> {
    fn sum(&self) -> T;
}

impl<T: Default + Copy + Add<Output = T>> Sum<T> for BlobTypeMap<T> {
    fn sum(&self) -> T {
        self.values().fold(T::default(), |acc, x| acc + *x)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Constructor)]
pub struct Blob {
    tpe: BlobType,
    id: Id,
}
