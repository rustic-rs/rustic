use derive_getters::Getters;
use serde::{Deserialize, Serialize};

use crate::backend::{FileType, RepoFile};
use crate::id::Id;

#[derive(Debug, Default, Serialize, Deserialize, Getters)]
pub struct ConfigFile {
    version: u32,
    id: Id,
    chunker_polynomial: String,
}

impl RepoFile for ConfigFile {
    const TYPE: FileType = FileType::Config;
}
