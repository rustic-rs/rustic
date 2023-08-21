use serde::{Deserialize, Serialize};

use crate::{
    backend::FileType, blob::BlobType, error::ConfigFileErrorKind, id::Id, repofile::RepoFile,
    RusticResult,
};

pub(super) mod constants {

    pub(super) const KB: u32 = 1024;
    pub(super) const MB: u32 = 1024 * KB;
    // default pack size
    pub(super) const DEFAULT_TREE_SIZE: u32 = 4 * MB;
    pub(super) const DEFAULT_DATA_SIZE: u32 = 32 * MB;
    // the default factor used for repo-size dependent pack size.
    // 32 * sqrt(reposize in bytes) = 1 MB * sqrt(reposize in GB)
    pub(super) const DEFAULT_GROW_FACTOR: u32 = 32;
    pub(super) const DEFAULT_SIZE_LIMIT: u32 = u32::MAX;
}

#[serde_with::apply(Option => #[serde(default, skip_serializing_if = "Option::is_none")])]
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct ConfigFile {
    pub version: u32,
    pub id: Id,
    pub chunker_polynomial: String,
    pub is_hot: Option<bool>,
    /// compression level
    ///
    /// Note: that `Some(0)` means no compression
    pub compression: Option<i32>,
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

impl ConfigFile {
    #[must_use]
    pub fn new(version: u32, id: Id, poly: u64) -> Self {
        Self {
            version,
            id,
            chunker_polynomial: format!("{poly:x}"),
            ..Self::default()
        }
    }

    pub fn poly(&self) -> RusticResult<u64> {
        Ok(u64::from_str_radix(&self.chunker_polynomial, 16)
            .map_err(ConfigFileErrorKind::ParsingFailedForPolynomial)?)
    }

    pub fn zstd(&self) -> RusticResult<Option<i32>> {
        match (self.version, self.compression) {
            (1, _) | (2, Some(0)) => Ok(None),
            (2, None) => Ok(Some(0)), // use default (=0) zstd compression
            (2, Some(c)) => Ok(Some(c)),
            _ => Err(ConfigFileErrorKind::ConfigVersionNotSupported.into()),
        }
    }

    #[must_use]
    pub fn packsize(&self, blob: BlobType) -> (u32, u32, u32) {
        match blob {
            BlobType::Tree => (
                self.treepack_size.unwrap_or(constants::DEFAULT_TREE_SIZE),
                self.treepack_growfactor
                    .unwrap_or(constants::DEFAULT_GROW_FACTOR),
                self.treepack_size_limit
                    .unwrap_or(constants::DEFAULT_SIZE_LIMIT),
            ),
            BlobType::Data => (
                self.datapack_size.unwrap_or(constants::DEFAULT_DATA_SIZE),
                self.datapack_growfactor
                    .unwrap_or(constants::DEFAULT_GROW_FACTOR),
                self.datapack_size_limit
                    .unwrap_or(constants::DEFAULT_SIZE_LIMIT),
            ),
        }
    }

    #[must_use]
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
