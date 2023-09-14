pub(crate) mod cache;
pub(crate) mod choose;
pub(crate) mod decrypt;
pub(crate) mod dry_run;
pub(crate) mod hotcold;
pub(crate) mod ignore;
pub(crate) mod local;
pub(crate) mod node;
pub(crate) mod rclone;
pub(crate) mod rest;
pub(crate) mod stdin;

use std::{io::Read, path::PathBuf};

use bytes::Bytes;
use log::trace;
use serde::{Deserialize, Serialize};

use crate::{backend::node::Node, error::BackendErrorKind, error::RusticResult, id::Id};

/// All [`FileType`]s which are located in separated directories
pub const ALL_FILE_TYPES: [FileType; 4] = [
    FileType::Key,
    FileType::Snapshot,
    FileType::Index,
    FileType::Pack,
];

/// Type for describing the kind of a file that can occur.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileType {
    /// Config file
    #[serde(rename = "config")]
    Config,
    /// Index
    #[serde(rename = "index")]
    Index,
    /// Keys
    #[serde(rename = "key")]
    Key,
    /// Snapshots
    #[serde(rename = "snapshot")]
    Snapshot,
    /// Data
    #[serde(rename = "pack")]
    Pack,
}

impl FileType {
    const fn dirname(self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::Snapshot => "snapshots",
            Self::Index => "index",
            Self::Key => "keys",
            Self::Pack => "data",
        }
    }

    /// Returns if the file type is cacheable.
    const fn is_cacheable(self) -> bool {
        match self {
            Self::Config | Self::Key | Self::Pack => false,
            Self::Snapshot | Self::Index => true,
        }
    }
}

/// Trait for backends that can read.
///
/// This trait is implemented by all backends that can read data.
pub trait ReadBackend: Clone + Send + Sync + 'static {
    /// Returns the location of the backend.
    fn location(&self) -> String;

    /// Sets an option of the backend.
    ///
    /// # Arguments
    ///
    /// * `option` - The option to set.
    /// * `value` - The value to set the option to.
    ///
    /// # Errors
    ///
    /// If the option is not supported.
    fn set_option(&mut self, option: &str, value: &str) -> RusticResult<()>;

    /// Lists all files with their size of the given type.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the files to list.
    ///
    /// # Errors
    ///
    /// If the files could not be listed.
    fn list_with_size(&self, tpe: FileType) -> RusticResult<Vec<(Id, u32)>>;

    /// Lists all files of the given type.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the files to list.
    ///
    /// # Errors
    ///
    /// If the files could not be listed.
    fn list(&self, tpe: FileType) -> RusticResult<Vec<Id>> {
        Ok(self
            .list_with_size(tpe)?
            .into_iter()
            .map(|(id, _)| id)
            .collect())
    }

    /// Reads full data of the given file.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `id` - The id of the file.
    ///
    /// # Errors
    ///
    /// If the file could not be read.
    fn read_full(&self, tpe: FileType, id: &Id) -> RusticResult<Bytes>;

    /// Reads partial data of the given file.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `id` - The id of the file.
    /// * `cacheable` - Whether the file should be cached.
    /// * `offset` - The offset to read from.
    /// * `length` - The length to read.
    ///
    /// # Errors
    ///
    /// If the file could not be read.
    fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        cacheable: bool,
        offset: u32,
        length: u32,
    ) -> RusticResult<Bytes>;

    /// Finds the id of the file starting with the given string.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The type of the strings.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `vec` - The strings to search for.
    ///
    /// # Errors
    ///
    /// * [`BackendErrorKind::NoSuitableIdFound`] - If no id could be found.
    /// * [`BackendErrorKind::IdNotUnique`] - If the id is not unique.
    ///
    /// # Note
    ///
    /// This function is used to find the id of a snapshot or index file.
    /// The id of a snapshot or index file is the id of the first pack file.
    ///
    /// [`BackendErrorKind::NoSuitableIdFound`]: crate::error::BackendErrorKind::NoSuitableIdFound
    /// [`BackendErrorKind::IdNotUnique`]: crate::error::BackendErrorKind::IdNotUnique
    fn find_starts_with<T: AsRef<str>>(&self, tpe: FileType, vec: &[T]) -> RusticResult<Vec<Id>> {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum MapResult<T> {
            None,
            Some(T),
            NonUnique,
        }
        let mut results = vec![MapResult::None; vec.len()];
        for id in self.list(tpe)? {
            let id_hex = id.to_hex();
            for (i, v) in vec.iter().enumerate() {
                if id_hex.starts_with(v.as_ref()) {
                    if results[i] == MapResult::None {
                        results[i] = MapResult::Some(id);
                    } else {
                        results[i] = MapResult::NonUnique;
                    }
                }
            }
        }

        results
            .into_iter()
            .enumerate()
            .map(|(i, id)| match id {
                MapResult::Some(id) => Ok(id),
                MapResult::None => {
                    Err(BackendErrorKind::NoSuitableIdFound((vec[i]).as_ref().to_string()).into())
                }
                MapResult::NonUnique => {
                    Err(BackendErrorKind::IdNotUnique((vec[i]).as_ref().to_string()).into())
                }
            })
            .collect()
    }

    /// Finds the id of the file starting with the given string.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `id` - The string to search for.
    ///
    /// # Errors
    ///
    /// * [`IdErrorKind::HexError`] - If the string is not a valid hexadecimal string
    /// * [`BackendErrorKind::NoSuitableIdFound`] - If no id could be found.
    /// * [`BackendErrorKind::IdNotUnique`] - If the id is not unique.
    ///
    /// [`IdErrorKind::HexError`]: crate::error::IdErrorKind::HexError
    /// [`BackendErrorKind::NoSuitableIdFound`]: crate::error::BackendErrorKind::NoSuitableIdFound
    /// [`BackendErrorKind::IdNotUnique`]: crate::error::BackendErrorKind::IdNotUnique
    fn find_id(&self, tpe: FileType, id: &str) -> RusticResult<Id> {
        Ok(self.find_ids(tpe, &[id.to_string()])?.remove(0))
    }

    /// Finds the ids of the files starting with the given strings.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The type of the strings.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `ids` - The strings to search for.
    ///
    /// # Errors
    ///
    /// * [`IdErrorKind::HexError`] - If the string is not a valid hexadecimal string
    /// * [`BackendErrorKind::NoSuitableIdFound`] - If no id could be found.
    /// * [`BackendErrorKind::IdNotUnique`] - If the id is not unique.
    ///
    /// [`IdErrorKind::HexError`]: crate::error::IdErrorKind::HexError
    /// [`BackendErrorKind::NoSuitableIdFound`]: crate::error::BackendErrorKind::NoSuitableIdFound
    /// [`BackendErrorKind::IdNotUnique`]: crate::error::BackendErrorKind::IdNotUnique
    fn find_ids<T: AsRef<str>>(&self, tpe: FileType, ids: &[T]) -> RusticResult<Vec<Id>> {
        ids.iter()
            .map(|id| Id::from_hex(id.as_ref()))
            .collect::<RusticResult<Vec<_>>>()
            .or_else(|err|{
                trace!("no valid IDs given: {err}, searching for ID starting with given strings instead");
                self.find_starts_with(tpe, ids)})
    }
}

/// Trait for backends that can write.
/// This trait is implemented by all backends that can write data.
pub trait WriteBackend: ReadBackend {
    /// Creates a new backend.
    fn create(&self) -> RusticResult<()>;

    /// Writes bytes to the given file.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `id` - The id of the file.
    /// * `cacheable` - Whether the data should be cached.
    /// * `buf` - The data to write.
    fn write_bytes(&self, tpe: FileType, id: &Id, cacheable: bool, buf: Bytes) -> RusticResult<()>;

    /// Removes the given file.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `id` - The id of the file.
    /// * `cacheable` - Whether the file is cacheable.
    fn remove(&self, tpe: FileType, id: &Id, cacheable: bool) -> RusticResult<()>;
}

/// Information about an entry to be able to open it.
///
/// # Type Parameters
///
/// * `O` - The type of the open information.
#[derive(Debug, Clone)]
pub struct ReadSourceEntry<O> {
    /// The path of the entry.
    pub path: PathBuf,

    /// The node information of the entry.
    pub node: Node,

    /// Information about how to open the entry.
    pub open: Option<O>,
}

/// Trait for backends that can read and open sources.
/// This trait is implemented by all backends that can read data and open from a source.
pub trait ReadSourceOpen {
    type Reader: Read + Send + 'static;

    /// Opens the source.
    fn open(self) -> RusticResult<Self::Reader>;
}

/// Trait for backends that can read from a source.
///
/// This trait is implemented by all backends that can read data from a source.
pub trait ReadSource {
    type Open: ReadSourceOpen;
    type Iter: Iterator<Item = RusticResult<ReadSourceEntry<Self::Open>>>;

    /// Returns the size of the source.
    fn size(&self) -> RusticResult<Option<u64>>;

    /// Returns an iterator over the entries of the source.
    fn entries(self) -> Self::Iter;
}

/// Trait for backends that can write to a source.
///
/// This trait is implemented by all backends that can write data to a source.
pub trait WriteSource: Clone {
    /// Create a new source.
    ///
    /// # Type Parameters
    ///
    /// * `P` - The type of the path.
    ///
    /// # Arguments
    ///
    /// * `path` - The path of the source.
    /// * `node` - The node information of the source.
    fn create<P: Into<PathBuf>>(&self, path: P, node: Node);

    /// Set the metadata of a source.
    ///
    /// # Type Parameters
    ///
    /// * `P` - The type of the path.
    ///
    /// # Arguments
    ///
    /// * `path` - The path of the source.
    /// * `node` - The node information of the source.
    fn set_metadata<P: Into<PathBuf>>(&self, path: P, node: Node);

    /// Write data to a source at the given offset.
    ///
    /// # Type Parameters
    ///
    /// * `P` - The type of the path.
    ///
    /// # Arguments
    ///
    /// * `path` - The path of the source.
    /// * `offset` - The offset to write at.
    /// * `data` - The data to write.
    fn write_at<P: Into<PathBuf>>(&self, path: P, offset: u64, data: Bytes);
}
