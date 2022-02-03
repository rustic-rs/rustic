use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::backend::{FileType, ReadBackend};
use crate::id::Id;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ConfigFile {
    version: u32,
    id: Id,
    chunker_polynomial: String,
}

impl ConfigFile {
    pub fn from_backend_no_id<B: ReadBackend>(b: B) -> Result<Self> {
        let data = b.read_full(FileType::Config, Id::default())?;
        Ok(serde_json::from_slice::<ConfigFile>(&data)?)
    }
}
