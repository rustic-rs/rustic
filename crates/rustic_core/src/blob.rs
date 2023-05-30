pub(crate) mod packer;
pub(crate) mod tree;

use std::{num::NonZeroU32, ops::Add};

use derive_more::Constructor;
use enum_map::{Enum, EnumMap};

use serde::{Deserialize, Serialize};

use crate::id::Id;

#[derive(Debug, Hash, PartialEq, Eq, Default, Clone, Copy)]
pub struct BlobLocation {
    pub offset: u32,
    pub length: u32,
    pub uncompressed_length: Option<NonZeroU32>,
}

impl BlobLocation {
    #[must_use]
    pub fn data_length(&self) -> u64 {
        self.uncompressed_length
            .map_or(self.length - 32, |length| length.get())
            .into()
    }
}

#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Enum,
)]
pub enum BlobType {
    #[serde(rename = "tree")]
    Tree,
    #[serde(rename = "data")]
    Data,
}

impl BlobType {
    #[must_use]
    pub const fn is_cacheable(self) -> bool {
        match self {
            Self::Tree => true,
            Self::Data => false,
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
        let mut btm = Self::default();
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
pub(crate) struct Blob {
    tpe: BlobType,
    id: Id,
}
