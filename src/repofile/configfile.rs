use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::backend::{FileType, RepoFile};
use crate::blob::BlobType;
use crate::id::Id;

#[serde_with::apply(Option => #[serde(default, skip_serializing_if = "Option::is_none")])]
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigFile {
    pub version: u32,
    pub id: Id,
    pub chunker_polynomial: String,
    pub is_hot: Option<bool>,
    pub compression: Option<i32>, // note that Some(0) means no compression.
    pub treepack_size: Option<u32>,
    pub treepack_growfactor: Option<u32>,
    pub treepack_size_limit: Option<u32>,
    pub datapack_size: Option<u32>,
    pub datapack_growfactor: Option<u32>,
    pub datapack_size_limit: Option<u32>,
    pub min_packsize_tolerate_percent: Option<u32>,
    pub max_packsize_tolerate_percent: Option<u32>,
}

impl RepoFile for ConfigFile {
    const TYPE: FileType = FileType::Config;
}

const KB: u32 = 1024;
const MB: u32 = 1024 * KB;
// default pack size
const DEFAULT_TREE_SIZE: u32 = 4 * MB;
const DEFAULT_DATA_SIZE: u32 = 32 * MB;
// the default factor used for repo-size dependent pack size.
// 32 * sqrt(reposize in bytes) = 1 MB * sqrt(reposize in GB)
const DEFAULT_GROW_FACTOR: u32 = 32;
const DEFAULT_SIZE_LIMIT: u32 = u32::MAX;

impl ConfigFile {
    pub fn new(version: u32, id: Id, poly: u64) -> Self {
        Self {
            version,
            id,
            chunker_polynomial: format!("{poly:x}"),
            ..Self::default()
        }
    }

    pub fn poly(&self) -> Result<u64> {
        Ok(u64::from_str_radix(&self.chunker_polynomial, 16)?)
    }

    pub fn zstd(&self) -> Result<Option<i32>> {
        match (self.version, self.compression) {
            (1, _) | (2, Some(0)) => Ok(None),
            (2, None) => Ok(Some(0)), // use default (=0) zstd compression
            (2, Some(c)) => Ok(Some(c)),
            _ => bail!("config version not supported!"),
        }
    }

    pub fn packsize(&self, blob: BlobType) -> (u32, u32, u32) {
        match blob {
            BlobType::Tree => (
                self.treepack_size.unwrap_or(DEFAULT_TREE_SIZE),
                self.treepack_growfactor.unwrap_or(DEFAULT_GROW_FACTOR),
                self.treepack_size_limit.unwrap_or(DEFAULT_SIZE_LIMIT),
            ),
            BlobType::Data => (
                self.datapack_size.unwrap_or(DEFAULT_DATA_SIZE),
                self.datapack_growfactor.unwrap_or(DEFAULT_GROW_FACTOR),
                self.datapack_size_limit.unwrap_or(DEFAULT_SIZE_LIMIT),
            ),
        }
    }

    pub fn packsize_ok_percents(&self) -> (u32, u32) {
        (
            self.min_packsize_tolerate_percent.unwrap_or(30),
            match self.max_packsize_tolerate_percent {
                None | Some(0) => u32::MAX,
                Some(percent) => percent,
            },
        )
    }
}
