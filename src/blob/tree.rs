use std::collections::HashSet;
use std::mem;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Local};
use derive_getters::Getters;
use serde::{Deserialize, Serialize};
use serde_aux::prelude::*;

use crate::backend::ReadBackend;
use crate::id::Id;
use crate::index::ReadIndex;

#[derive(Clone, Debug, Serialize, Deserialize, Getters)]
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
    pub fn is_tree(&self) -> bool {
        &self.tpe == "dir"
    }

    pub fn is_file(&self) -> bool {
        &self.tpe == "file"
    }

    pub fn is_symlink(&self) -> bool {
        &self.tpe == "symlink"
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

/// tree_iterator creates an Iterator over the trees given by ids using the backend be and the index
/// index
pub fn tree_iterator<'a>(
    be: &'a impl ReadBackend,
    index: &'a impl ReadIndex,
    ids: Vec<Id>,
) -> impl Iterator<Item = (PathBuf, Node)> + 'a {
    TreeIterator::new(
        |i| Tree::from_backend(be, index, i).unwrap().nodes.into_iter(),
        ids,
    )
}

/// tree_iterator_once creates an Iterator over the trees given by ids using the backend be and the index
/// index where each node is only visited once
pub fn tree_iterator_once<'a>(
    be: &'a impl ReadBackend,
    index: &'a impl ReadIndex,
    ids: Vec<Id>,
) -> impl Iterator<Item = (PathBuf, Node)> + 'a {
    let mut visited = HashSet::new();
    TreeIterator::new(
        move |i| {
            if visited.insert(i) {
                Tree::from_backend(be, index, i).unwrap().nodes.into_iter()
            } else {
                Vec::new().into_iter()
            }
        },
        ids,
    )
}

/// TreeIterator is a recursive iterator over a Tree, i.e. it recursively iterates over
/// a full tree visiting subtrees
pub struct TreeIterator<IT, F>
where
    IT: Iterator<Item = Node>,
    F: FnMut(Id) -> IT,
{
    open_iterators: Vec<IT>,
    inner: IT,
    path: PathBuf,
    getter: F,
}

impl<IT, F> TreeIterator<IT, F>
where
    IT: Iterator<Item = Node>,
    F: FnMut(Id) -> IT,
{
    fn new(mut getter: F, ids: Vec<Id>) -> Self {
        let mut iters = ids.into_iter().map(&mut getter).collect::<Vec<_>>();
        iters.rotate_right(1);
        Self {
            inner: iters.pop().unwrap(),
            open_iterators: iters,
            path: PathBuf::new(),
            getter,
        }
    }
}

impl<IT, F> Iterator for TreeIterator<IT, F>
where
    IT: Iterator<Item = Node>,
    F: FnMut(Id) -> IT,
{
    type Item = (PathBuf, Node);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                Some(node) => {
                    let path = self.path.join(node.name.clone());
                    if node.is_tree() {
                        let old_inner = mem::replace(&mut self.inner, (self.getter)(node.subtree));
                        self.open_iterators.push(old_inner);
                        self.path.push(node.name.clone());
                    }

                    return Some((path, node));
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
