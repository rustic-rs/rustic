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
use displaydoc::Display;
use log::trace;
use serde::{Deserialize, Serialize};

use crate::{backend::node::Node, error::BackendErrorKind, id::Id, RusticResult};

/// All [`FileType`]s which are located in separated directories
pub const ALL_FILE_TYPES: [FileType; 4] = [
    FileType::Key,
    FileType::Snapshot,
    FileType::Index,
    FileType::Pack,
];

/// Type for describing the kind of a file that can occur.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum FileType {
    /// config
    #[serde(rename = "config")]
    Config,
    /// index
    #[serde(rename = "index")]
    Index,
    /// keys
    #[serde(rename = "key")]
    Key,
    /// snapshots
    #[serde(rename = "snapshot")]
    Snapshot,
    /// data
    #[serde(rename = "pack")]
    Pack,
}

impl From<FileType> for &'static str {
    fn from(value: FileType) -> &'static str {
        match value {
            FileType::Config => "config",
            FileType::Snapshot => "snapshots",
            FileType::Index => "index",
            FileType::Key => "keys",
            FileType::Pack => "data",
        }
    }
}

impl FileType {
    const fn is_cacheable(self) -> bool {
        match self {
            Self::Config | Self::Key | Self::Pack => false,
            Self::Snapshot | Self::Index => true,
        }
    }
}

pub trait ReadBackend: Clone + Send + Sync + 'static {
    fn location(&self) -> String;

    fn set_option(&mut self, option: &str, value: &str) -> RusticResult<()>;

    fn list_with_size(&self, tpe: FileType) -> RusticResult<Vec<(Id, u32)>>;

    fn list(&self, tpe: FileType) -> RusticResult<Vec<Id>> {
        Ok(self
            .list_with_size(tpe)?
            .into_iter()
            .map(|(id, _)| id)
            .collect())
    }

    fn read_full(&self, tpe: FileType, id: &Id) -> RusticResult<Bytes>;

    fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        cacheable: bool,
        offset: u32,
        length: u32,
    ) -> RusticResult<Bytes>;

    fn find_starts_with(&self, tpe: FileType, vec: &[String]) -> RusticResult<Vec<Id>> {
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
                if id_hex.starts_with(v) {
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
                    Err(BackendErrorKind::NoSuitableIdFound((vec[i]).clone()).into())
                }
                MapResult::NonUnique => Err(BackendErrorKind::IdNotUnique((vec[i]).clone()).into()),
            })
            .collect()
    }

    fn find_id(&self, tpe: FileType, id: &str) -> RusticResult<Id> {
        Ok(self.find_ids(tpe, &[id.to_string()])?.remove(0))
    }

    fn find_ids(&self, tpe: FileType, ids: &[String]) -> RusticResult<Vec<Id>> {
        ids.iter()
            .map(|id| Id::from_hex(id))
            .collect::<RusticResult<Vec<_>>>()
            .or_else(|err|{
                trace!("no valid IDs given: {err}, searching for ID starting with given strings instead");
                self.find_starts_with(tpe, ids)})
    }
}

pub trait WriteBackend: ReadBackend {
    fn create(&self) -> RusticResult<()>;

    fn write_bytes(&self, tpe: FileType, id: &Id, cacheable: bool, buf: Bytes) -> RusticResult<()>;

    fn remove(&self, tpe: FileType, id: &Id, cacheable: bool) -> RusticResult<()>;
}

#[derive(Debug, Clone)]
pub struct ReadSourceEntry<O> {
    pub path: PathBuf,
    pub node: Node,
    pub open: Option<O>,
}

pub trait ReadSourceOpen {
    type Reader: Read + Send + 'static;

    fn open(self) -> RusticResult<Self::Reader>;
}

pub trait ReadSource {
    type Open: ReadSourceOpen;
    type Iter: Iterator<Item = RusticResult<ReadSourceEntry<Self::Open>>>;

    fn size(&self) -> RusticResult<Option<u64>>;
    fn entries(self) -> Self::Iter;
}

pub trait WriteSource: Clone {
    fn create<P: Into<PathBuf>>(&self, path: P, node: Node);
    fn set_metadata<P: Into<PathBuf>>(&self, path: P, node: Node);
    fn write_at<P: Into<PathBuf>>(&self, path: P, offset: u64, data: Bytes);
}
