use std::ffi::OsString;
use std::fmt::Debug;
use std::path::PathBuf;

use chrono::{DateTime, Local};
use derive_getters::Getters;
use derive_more::{Constructor, IsVariant};
use serde::{Deserialize, Serialize};
use serde_aux::prelude::*;

use crate::id::Id;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Constructor)]
pub struct Node {
    name: String,
    #[serde(flatten)]
    node_type: NodeType,
    #[serde(flatten)]
    meta: Metadata,
    #[serde(default, deserialize_with = "deserialize_default_from_null")]
    content: Option<Vec<Id>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    subtree: Option<Id>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, IsVariant)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum NodeType {
    File,
    Dir,
    Symlink { linktarget: String },
    Dev { device: u64 },
    Chardev { device: u64 },
    Fifo,
    Socket,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, Getters)]
pub struct Metadata {
    #[serde(default, skip_serializing_if = "is_default")]
    pub mode: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mtime: Option<DateTime<Local>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atime: Option<DateTime<Local>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ctime: Option<DateTime<Local>>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub uid: u32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub gid: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub inode: u64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub device_id: u64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub size: u64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub links: u64,
}

fn is_default<T: Default + PartialEq>(t: &T) -> bool {
    t == &T::default()
}

impl Node {
    pub fn new_file(name: OsString, meta: Metadata) -> Self {
        Self {
            name: name.to_str().expect("no unicode").to_string(),
            node_type: NodeType::File,
            content: None,
            subtree: None,
            meta,
        }
    }

    pub fn new_dir(name: OsString, meta: Metadata) -> Self {
        Self {
            name: name.to_str().expect("no unicode").to_string(),
            node_type: NodeType::Dir,
            content: None,
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
            content: None,
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
        self.content = Some(content);
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn node_type(&self) -> &NodeType {
        &self.node_type
    }

    pub fn meta(&self) -> &Metadata {
        &self.meta
    }

    pub fn content(&self) -> &Vec<Id> {
        self.content.as_ref().unwrap()
    }

    pub fn subtree(&self) -> &Option<Id> {
        &self.subtree
    }
}
