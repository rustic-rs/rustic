use std::ffi::OsString;
use std::fmt::Debug;
use std::path::PathBuf;

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
    #[serde(
        default,
        deserialize_with = "deserialize_default_from_null",
        skip_serializing_if = "Vec::is_empty"
    )]
    content: Vec<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, Constructor, Getters)]
pub struct Metadata {
    #[serde(default, skip_serializing_if = "is_default")]
    size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mtime: Option<DateTime<Local>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    atime: Option<DateTime<Local>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ctime: Option<DateTime<Local>>,
    #[serde(default, skip_serializing_if = "is_default")]
    mode: u32,
    #[serde(default, skip_serializing_if = "is_default")]
    uid: u32,
    #[serde(default, skip_serializing_if = "is_default")]
    gid: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    group: Option<String>,
    #[serde(default, skip_serializing_if = "is_default")]
    inode: u64,
    #[serde(default, skip_serializing_if = "is_default")]
    device_id: u64,
    #[serde(default, skip_serializing_if = "is_default")]
    links: u64,
}

fn is_default<T: Default + PartialEq>(t: &T) -> bool {
    t == &T::default()
}

impl Node {
    pub fn new_file(name: OsString, meta: Metadata) -> Self {
        Self {
            name: name.to_str().expect("no unicode").to_string(),
            node_type: NodeType::File,
            content: Vec::new(),
            subtree: None,
            meta,
        }
    }

    pub fn new_dir(name: OsString, meta: Metadata) -> Self {
        Self {
            name: name.to_str().expect("no unicode").to_string(),
            node_type: NodeType::Dir,
            content: Vec::new(),
            subtree: None,
            meta,
        }
    }

    pub fn new_symlink(name: OsString, target: PathBuf, meta: Metadata) -> Self {
        Self {
            name: name.to_str().expect("no unicode").to_string(),
            node_type: NodeType::Symlink {
                linktarget: target.to_str().expect("no unicode").to_string(),
            },
            content: Vec::new(),
            subtree: None,
            meta,
        }
    }

    pub fn is_dir(&self) -> bool {
        self.node_type == NodeType::Dir
    }

    pub fn set_subtree(&mut self, id: Id) {
        self.subtree = Some(id);
    }

    pub fn set_content(&mut self, content: Vec<Id>) {
        self.content = content;
    }
}
