use anyhow::Result;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::Id;
use crate::backend::{FileType, ReadBackend};

#[derive(Debug, Serialize, Deserialize)]
pub struct SnapshotFile {
    pub time: DateTime<Local>,
    pub tree: Id,
    pub paths: Vec<PathBuf>,
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub uid: u32,
    #[serde(default)]
    pub gid: u32,
    #[serde(default)]
    pub tags: TagList,
}

impl SnapshotFile {
    /// Get an IndexFile from the backend
    pub fn from_backend<B: ReadBackend>(be: &B, id: Id) -> Result<Self> {
        let data = be.read_full(FileType::Snapshot, id)?;
        Ok(serde_json::from_slice(&data)?)
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct TagList(Vec<Tag>);

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Tag(String);
