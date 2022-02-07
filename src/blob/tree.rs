use std::mem;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Local};
use derive_more::Constructor;
use serde::{Deserialize, Serialize};
use serde_aux::prelude::*;

use crate::backend::ReadBackend;
use crate::id::Id;
use crate::index::ReadIndex;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    name: String,
    #[serde(rename = "type")]
    tpe: String,
    #[serde(default)]
    mode: u32,
    mtime: DateTime<Local>,
    atime: DateTime<Local>,
    ctime: DateTime<Local>,
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
    size: u64,
    #[serde(default)]
    links: u64,
    #[serde(default)]
    linktarget: String,
    #[serde(default)]
    device: u64,
    #[serde(deserialize_with = "deserialize_default_from_null")]
    content: Vec<Id>,
    #[serde(default)]
    subtree: Id,
}

impl Node {
    fn is_tree(&self) -> bool {
        &self.tpe == "dir"
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tree {
    nodes: Vec<Node>,
}

impl Tree {
    pub fn from_backend(be: &impl ReadBackend, index: &impl ReadIndex, id: Id) -> Result<Self> {
        let data = index
            .get_id(&id)
            .ok_or(anyhow!("blob not found in index"))?
            .read_data(be)?;

        Ok(serde_json::from_slice(&data)?)
    }
}

/// TreeIterator is a recursive iterator over a Tree, i.e. it recursively iterates over
/// a full tree visiting subtrees
#[derive(Constructor)]
pub struct TreeIterator<BE, I, IT>
where
    BE: ReadBackend,
    I: ReadIndex,
    IT: Iterator<Item = Node>,
{
    be: BE,
    index: I,
    open_iterators: Vec<IT>,
    inner: IT,
    path: PathBuf,
}

impl<BE, I> TreeIterator<BE, I, std::vec::IntoIter<Node>>
where
    BE: ReadBackend,
    I: ReadIndex,
{
    pub fn from_id(be: BE, index: I, id: Id) -> Self {
        Self {
            inner: Tree::from_backend(&be, &index, id)
                .unwrap()
                .nodes
                .into_iter(),
            be,
            index,
            open_iterators: Vec::new(),
            path: PathBuf::new(),
        }
    }
}

pub struct PathNode {
    pub path: PathBuf,
    pub node: Node,
}

impl<BE, I> Iterator for TreeIterator<BE, I, std::vec::IntoIter<Node>>
where
    BE: ReadBackend,
    I: ReadIndex,
{
    type Item = PathNode;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                Some(node) => {
                    if node.is_tree() {
                        let new_inner = Tree::from_backend(&self.be, &self.index, node.subtree)
                            .unwrap()
                            .nodes
                            .into_iter();
                        let old_inner = mem::replace(&mut self.inner, new_inner);
                        self.open_iterators.push(old_inner);
                        self.path.push(node.name.clone());
                    }
                    return Some(PathNode {
                        path: if node.is_tree() {
                            self.path.clone()
                        } else {
                            self.path.join(&node.name)
                        },
                        node,
                    });
                }
                None => match self.open_iterators.pop() {
                    Some(it) => {
                        self.inner = it;
                        self.path.pop();
                    }
                    None => return None,
                },
            }
        }
    }
}
