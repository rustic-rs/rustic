use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::backend::{FileType, RepoFile};
use crate::id::Id;

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigFile {
    pub version: u32,
    pub id: Id,
    pub chunker_polynomial: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_hot: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compression: Option<i32>, // note that Some(0) means no compression.
}

impl RepoFile for ConfigFile {
    const TYPE: FileType = FileType::Config;
}

impl ConfigFile {
    pub fn new(version: u32, id: Id, poly: u64) -> Self {
        Self {
            version,
            id,
            chunker_polynomial: format!("{:x}", poly),
            is_hot: None,
            compression: None,
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
}
