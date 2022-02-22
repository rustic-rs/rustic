use std::ffi::OsString;
use std::fmt::Debug;

use anyhow::Result;
use chrono::{DateTime, Local};
use derive_getters::Getters;
use derive_more::{Constructor, IsVariant};
use serde::{Deserialize, Serialize};
use serde_aux::prelude::*;

use crate::id::Id;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Constructor, Getters)]
pub struct Node {
    name: String,
    #[serde(flatten)]
    node_type: NodeType,
    #[serde(deserialize_with = "deserialize_default_from_null")]
    content: Vec<Id>,
    subtree: Option<Id>,
    #[serde(flatten)]
    meta: Metadata,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, IsVariant)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum NodeType {
    File,
    Dir,
    Symlink { linktarget: String },
    Device { device: u64 },
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, Getters)]
pub struct Metadata {
    #[serde(default)]
    size: u64,
    mtime: Option<DateTime<Local>>,
    atime: Option<DateTime<Local>>,
    ctime: Option<DateTime<Local>>,
    #[serde(default)]
    mode: u32,
    #[serde(default)]
    uid: u32,
    #[serde(default)]
    gid: u32,
    #[serde(default)]
    user: String,
    #[serde(default)]
    group: String,
    #[serde(default)]
    inode: u64,
    #[serde(default)]
    device_id: u64,
    #[serde(default)]
    links: u64,
}

impl Node {
    pub fn from_content(name: OsString, content: Vec<Id>, _size: u64) -> Result<Self> {
        Ok(Self {
            name: name.to_str().expect("no unicode").to_string(),
            node_type: NodeType::File,
            content,
            subtree: None,
            meta: Metadata::default(),
        })
    }

    pub fn from_tree(name: OsString, id: Id) -> Result<Self> {
        Ok(Self {
            name: name.to_str().expect("no unicode").to_string(),
            node_type: NodeType::Dir,
            content: Vec::new(),
            subtree: Some(id),
            meta: Metadata::default(),
        })
    }
}
