pub(crate) mod packer;
pub(crate) mod tree;

use derive_more::Constructor;
use enum_map::{Enum, EnumMap};
use serde::{Deserialize, Serialize};

use crate::id::Id;

/// All [`BlobType`]s which are supported by the repository
pub const ALL_BLOB_TYPES: [BlobType; 2] = [BlobType::Tree, BlobType::Data];

#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Enum,
)]
/// The type a `blob` or a `packfile` can have
pub enum BlobType {
    #[serde(rename = "tree")]
    /// This is a tree blob
    Tree,
    #[serde(rename = "data")]
    /// This is a data blob
    Data,
}

impl BlobType {
    /// Defines the cacheability of a [`BlobType`]
    ///
    /// # Returns
    ///
    /// `true` if the [`BlobType`] is cacheable, `false` otherwise
    #[must_use]
    pub(crate) const fn is_cacheable(self) -> bool {
        match self {
            Self::Tree => true,
            Self::Data => false,
        }
    }
}

pub type BlobTypeMap<T> = EnumMap<BlobType, T>;

/// Initialize is a new trait to define the method init() for a [`BlobTypeMap`]
pub trait Initialize<T: Default + Sized> {
    /// Initialize a [`BlobTypeMap`] by processing a given function for each [`BlobType`]
    fn init<F: FnMut(BlobType) -> T>(init: F) -> BlobTypeMap<T>;
}

impl<T: Default> Initialize<T> for BlobTypeMap<T> {
    /// Initialize a [`BlobTypeMap`] by processing a given function for each [`BlobType`]
    ///
    /// # Arguments
    ///
    /// * `init` - The function to process for each [`BlobType`]
    ///
    /// # Returns
    ///
    /// A [`BlobTypeMap`] with the result of the function for each [`BlobType`]
    fn init<F: FnMut(BlobType) -> T>(mut init: F) -> Self {
        let mut btm = Self::default();
        for i in 0..BlobType::LENGTH {
            let bt = BlobType::from_usize(i);
            btm[bt] = init(bt);
        }
        btm
    }
}

/// A `Blob` is a file that is stored in the backend.
///
/// It can be a `tree` or a `data` blob.
///
/// A `tree` blob is a file that contains a list of other blobs.
/// A `data` blob is a file that contains the actual data.
#[derive(Debug, PartialEq, Eq, Clone, Constructor)]
pub(crate) struct Blob {
    /// The type of the blob
    tpe: BlobType,

    /// The id of the blob
    id: Id,
}
