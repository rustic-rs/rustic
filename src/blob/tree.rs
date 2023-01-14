use std::collections::HashSet;
use std::ffi::OsStr;
use std::mem;
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, bail, Result};
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use derive_getters::Getters;
use indicatif::ProgressBar;
use serde::{Deserialize, Deserializer, Serialize};

use crate::crypto::hash;
use crate::id::Id;
use crate::index::IndexedBackend;

use super::{Metadata, Node, NodeType};

#[derive(Clone, Debug, Serialize, Deserialize, Getters)]
pub struct Tree {
    #[serde(deserialize_with = "deserialize_null_default")]
    nodes: Vec<Node>,
}

fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    T: Default + Deserialize<'de>,
    D: Deserializer<'de>,
{
    let opt = Option::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

impl Tree {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    pub fn add(&mut self, node: Node) {
        self.nodes.push(node)
    }

    pub fn serialize(&self) -> Result<(Vec<u8>, Id)> {
        let mut chunk = serde_json::to_vec(&self)?;
        chunk.push(b'\n'); // for whatever reason, restic adds a newline, so to be compatible...
        let id = hash(&chunk);
        Ok((chunk, id))
    }

    pub fn from_backend(be: &impl IndexedBackend, id: Id) -> Result<Self> {
        let data = be
            .get_tree(&id)
            .ok_or_else(|| anyhow!("blob {id:?} not found in index"))?
            .read_data(be.be())?;

        Ok(serde_json::from_slice(&data)?)
    }

    pub fn node_from_path(be: &impl IndexedBackend, id: Id, path: &Path) -> Result<Node> {
        let mut node = Node::new_node(OsStr::new(""), NodeType::Dir, Metadata::default());
        node.set_subtree(id);
        for p in path.components() {
            match p {
                Component::RootDir | Component::Prefix(_) => {}
                Component::Normal(p) => {
                    let id = node.subtree().ok_or_else(|| anyhow!("{p:?} is no dir"))?;
                    let tree = Tree::from_backend(be, id)?;
                    node = tree
                        .nodes
                        .into_iter()
                        .find(|node| node.name() == p)
                        .ok_or_else(|| anyhow!("{p:?} not found"))?;
                }
                _ => bail!("path should not contain current or parent dir, path: {path:?}"),
            }
        }
        Ok(node)
    }
}

impl IntoIterator for Tree {
    type Item = Node;
    type IntoIter = std::vec::IntoIter<Node>;

    fn into_iter(self) -> Self::IntoIter {
        self.nodes.into_iter()
    }
}

/// NodeStreamer recursively streams all nodes of a given tree including all subtrees in-order
pub struct NodeStreamer<BE>
where
    BE: IndexedBackend,
{
    open_iterators: Vec<std::vec::IntoIter<Node>>,
    inner: std::vec::IntoIter<Node>,
    path: PathBuf,
    be: BE,
}

impl<BE> NodeStreamer<BE>
where
    BE: IndexedBackend,
{
    pub fn new(be: BE, node: &Node) -> Result<Self> {
        let inner = if node.is_dir() {
            Tree::from_backend(&be, node.subtree.unwrap())?
                .nodes
                .into_iter()
        } else {
            vec![node.clone()].into_iter()
        };
        Ok(Self {
            inner,
            open_iterators: Vec::new(),
            path: PathBuf::new(),
            be,
        })
    }
}

type NodeStreamItem = Result<(PathBuf, Node)>;

// TODO: This is not parallel at the moment...
impl<BE> Iterator for NodeStreamer<BE>
where
    BE: IndexedBackend,
{
    type Item = NodeStreamItem;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                Some(node) => {
                    let path = self.path.join(node.name());
                    if let Some(id) = node.subtree() {
                        self.path.push(node.name());
                        let be = self.be.clone();
                        let tree = match Tree::from_backend(&be, *id) {
                            Ok(tree) => tree,
                            Err(err) => return Some(Err(err)),
                        };
                        let old_inner = mem::replace(&mut self.inner, tree.nodes.into_iter());
                        self.open_iterators.push(old_inner);
                    }

                    return Some(Ok((path, node)));
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

/// TreeStreamerOnce recursively visits all trees and subtrees, but each tree ID only once
pub struct TreeStreamerOnce {
    visited: HashSet<Id>,
    queue_in: Option<Sender<(PathBuf, Id, usize)>>,
    queue_out: Receiver<Result<(PathBuf, Tree, usize)>>,
    p: ProgressBar,
    counter: Vec<usize>,
    finished_ids: usize,
}

const MAX_TREE_LOADER: usize = 4;

impl TreeStreamerOnce {
    pub fn new<BE: IndexedBackend>(be: BE, ids: Vec<Id>, p: ProgressBar) -> Result<Self> {
        p.set_length(ids.len() as u64);

        let (out_tx, out_rx) = bounded(MAX_TREE_LOADER);
        let (in_tx, in_rx) = unbounded();

        for _ in 0..MAX_TREE_LOADER {
            let be = be.clone();
            let in_rx = in_rx.clone();
            let out_tx = out_tx.clone();
            std::thread::spawn(move || {
                for (path, id, count) in in_rx {
                    out_tx
                        .send(Tree::from_backend(&be, id).map(|tree| (path, tree, count)))
                        .unwrap();
                }
            });
        }

        let counter = vec![0; ids.len()];
        let mut streamer = Self {
            visited: HashSet::new(),
            queue_in: Some(in_tx),
            queue_out: out_rx,
            p,
            counter,
            finished_ids: 0,
        };

        for (count, id) in ids.into_iter().enumerate() {
            if !streamer.add_pending(PathBuf::new(), id, count)? {
                streamer.p.inc(1);
                streamer.finished_ids += 1;
            }
        }

        Ok(streamer)
    }

    fn add_pending(&mut self, path: PathBuf, id: Id, count: usize) -> Result<bool> {
        if self.visited.insert(id) {
            self.queue_in.as_ref().unwrap().send((path, id, count))?;
            self.counter[count] += 1;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

type TreeStreamItem = Result<(PathBuf, Tree)>;

impl Iterator for TreeStreamerOnce {
    type Item = TreeStreamItem;

    fn next(&mut self) -> Option<Self::Item> {
        if self.counter.len() == self.finished_ids {
            drop(self.queue_in.take());
            self.p.finish();
            return None;
        }
        let (path, tree, count) = match self.queue_out.recv() {
            Ok(Ok(res)) => res,
            Err(err) => return Some(Err(err.into())),
            Ok(Err(err)) => return Some(Err(err)),
        };

        for node in tree.nodes() {
            if let Some(id) = node.subtree() {
                let mut path = path.clone();
                path.push(node.name());
                match self.add_pending(path, *id, count) {
                    Ok(_) => {}
                    Err(err) => return Some(Err(err)),
                }
            }
        }
        self.counter[count] -= 1;
        if self.counter[count] == 0 {
            self.p.inc(1);
            self.finished_ids += 1;
        }
        Some(Ok((path, tree)))
    }
}
