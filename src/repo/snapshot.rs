use std::path::PathBuf;

use super::Id;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SnapshotFile {
    time: DateTime<Local>,
    tree: Id,
    paths: Vec<PathBuf>,
    #[serde(default)]
    hostname: String,
    #[serde(default)]
    username: String,
    #[serde(default)]
    uid: u32,
    #[serde(default)]
    gid: u32,
    #[serde(default)]
    tags: TagList,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct TagList(Vec<Tag>);

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Tag(String);
