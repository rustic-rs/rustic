use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
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

pub trait RepoFile: Serialize + DeserializeOwned + Sized + Send + Sync + 'static {
    const TYPE: FileType;
}

#[async_trait]
pub trait ReadBackend: Clone + Send + Sync + 'static {
    type Error: Send + Sync + std::error::Error + 'static;

    fn location(&self) -> &str;
    async fn list_with_size(&self, tpe: FileType) -> Result<Vec<(Id, u32)>, Self::Error>;

    async fn list(&self, tpe: FileType) -> Result<Vec<Id>, Self::Error> {
        Ok(self
            .list_with_size(tpe)
            .await?
            .into_iter()
            .map(|(id, _)| id)
            .collect())
    }

    async fn read_full(&self, tpe: FileType, id: &Id) -> Result<Vec<u8>, Self::Error>;
    async fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>, Self::Error>;

    async fn find_starts_with(
        &self,
        tpe: FileType,
        vec: &[&str],
    ) -> Result<Vec<Result<Id, anyhow::Error>>, Self::Error> {
        let mut results = vec![MapResult::None; vec.len()];
        for id in self.list(tpe).await? {
            for (i, v) in vec.iter().enumerate() {
                if id.to_hex().starts_with(v) {
                    if results[i] == MapResult::None {
                        results[i] = MapResult::Some(id);
                    } else {
                        results[i] = MapResult::NonUnique;
                    }
                }
            }
        }

        Ok(results
            .into_iter()
            .enumerate()
            .map(|(i, id)| match id {
                MapResult::Some(id) => Ok(id),
                MapResult::None => Err(anyhow!("no suitable id found for {}", &vec[i])),
                MapResult::NonUnique => Err(anyhow!("id {} is not unique", &vec[i])),
            })
            .collect())
    }

    async fn find_id(&self, tpe: FileType, id: &str) -> Result<Id, anyhow::Error> {
        Ok(match Id::from_hex(id) {
            Ok(id) => id,
            // if the given id param is not a full Id, search for a suitable one
            Err(_) => self.find_starts_with(tpe, &[&id]).await?.remove(0)?,
        })
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum MapResult<T> {
    None,
    Some(T),
    NonUnique,
}

#[async_trait]
pub trait WriteBackend: Clone + Send + Sync + 'static {
    type Error: Send + Sync + std::error::Error + 'static;

    async fn write_full(
        &self,
        tpe: FileType,
        id: &Id,
        r: &mut (impl Read + Send + Sync),
    ) -> Result<(), Self::Error>;
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
