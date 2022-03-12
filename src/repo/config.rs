use anyhow::Result;
use derive_getters::Getters;
use serde::{Deserialize, Serialize};

use crate::backend::{DecryptReadBackend, FileType};
use crate::id::Id;

#[derive(Debug, Default, Serialize, Deserialize, Getters)]
pub struct ConfigFile {
    version: u32,
    id: Id,
    chunker_polynomial: String,
}

impl ConfigFile {
    pub fn from_backend_no_id<B: DecryptReadBackend>(b: &B) -> Result<Self> {
        let data = b.read_encrypted_full(FileType::Config, &Id::default())?;
        Ok(serde_json::from_slice::<ConfigFile>(&data)?)
    }
}
