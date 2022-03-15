use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use serde::{de::DeserializeOwned, Serialize};

use crate::id::Id;

pub mod decrypt;
pub mod dry_run;
pub mod ignore;
pub mod local;
pub mod node;

pub use self::ignore::*;
pub use decrypt::*;
pub use dry_run::*;
pub use local::*;
use node::Node;

#[derive(Clone, Copy, Debug)]
pub enum FileType {
    Config,
    Index,
    Key,
    Snapshot,
    Pack,
}

impl FileType {
    pub fn name(&self) -> &str {
        match &self {
            FileType::Config => "config",
            FileType::Snapshot => "snapshots",
            FileType::Index => "index",
            FileType::Key => "keys",
            FileType::Pack => "data",
        }
    }
}

pub trait RepoFile: Serialize + DeserializeOwned + Sized {
    const TYPE: FileType;
}

pub trait ReadBackend: Clone {
    type Error: Send + Sync + std::error::Error + 'static;

    fn location(&self) -> &str;
    fn list_with_size(&self, tpe: FileType) -> Result<Vec<(Id, u32)>, Self::Error>;

    fn list(&self, tpe: FileType) -> Result<Vec<Id>, Self::Error> {
        Ok(self
            .list_with_size(tpe)?
            .into_iter()
            .map(|(id, _)| id)
            .collect())
    }

    fn read_full(&self, tpe: FileType, id: &Id) -> Result<Vec<u8>, Self::Error>;
    fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>, Self::Error>;

    fn find_starts_with(
        &self,
        tpe: FileType,
        vec: &[&str],
    ) -> Result<Vec<Result<Id, anyhow::Error>>, Self::Error> {
        self.map_list(tpe, vec, |s, id| id.to_hex().starts_with(s))
            .map(|res| {
                res.into_iter()
                    .enumerate()
                    .map(|(i, id)| match id {
                        MapResult::Some(id) => Ok(id),
                        MapResult::None => Err(anyhow!("no suitable id found for {}", &vec[i])),
                        MapResult::NonUnique => Err(anyhow!("id {} is not unique", &vec[i])),
                    })
                    .collect()
            })
    }

    /// map_list
    fn map_list<T>(
        &self,
        tpe: FileType,
        vec: &[T],
        matches: impl Fn(&T, Id) -> bool,
    ) -> Result<Vec<MapResult<Id>>, Self::Error> {
        let mut res = vec![MapResult::None; vec.len()];
        for id in self.list(tpe)? {
            for (i, v) in vec.iter().enumerate() {
                if matches(v, id) {
                    if res[i] == MapResult::None {
                        res[i] = MapResult::Some(id);
                    } else {
                        res[i] = MapResult::NonUnique;
                    }
                }
            }
        }

        Ok(res)
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum MapResult<T> {
    None,
    Some(T),
    NonUnique,
}

pub trait WriteBackend: Clone {
    type Error: Send + Sync + std::error::Error + 'static;

    fn write_full(&self, tpe: FileType, id: &Id, r: &mut impl Read) -> Result<(), Self::Error>;
}

pub trait ReadSource: Iterator<Item = Result<(PathBuf, Node)>> {
    type Reader: Read;
    fn read(path: &Path) -> Result<Self::Reader>;
    fn size(&self) -> Result<u64>;
}

pub trait WriteSource: Clone {
    fn create(&self, path: PathBuf, node: Node);
    fn set_metadata(&self, path: PathBuf, node: Node);
    fn write_at(&self, path: PathBuf, offset: u64, data: Vec<u8>);
}
