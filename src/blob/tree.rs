use std::collections::{HashSet, VecDeque};
use std::mem;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use anyhow::{anyhow, Result};
use derive_getters::Getters;
use futures::{
    stream::FuturesUnordered,
    task::{Context, Poll},
    Future, Stream,
};
use indicatif::ProgressBar;
use serde::{Deserialize, Deserializer, Serialize};
use tokio::{spawn, task::JoinHandle};

use crate::crypto::hash;
use crate::id::Id;
use crate::index::IndexedBackend;

use super::Node;

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
            .ok_or_else(|| anyhow!("blob {} not found in index", id.to_hex()))?
            .read_data(be.be())?;

        Ok(serde_json::from_slice(&data)?)
    }

    pub fn subtree_id(be: &impl IndexedBackend, mut id: Id, path: &Path) -> Result<Id> {
        for p in path.iter() {
            let p = p.to_str().unwrap();
            // TODO: check for root instead
            if p == "/" {
                continue;
            }
            let tree = Tree::from_backend(be, id)?;
            let node = tree
                .nodes()
                .iter()
                .find(|node| node.name() == p)
                .ok_or_else(|| anyhow!("{} not found", p))?;
            id = node.subtree().ok_or_else(|| anyhow!("{} is no dir", p))?;
        }
        Ok(id)
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
    BE: IndexedBackend + Unpin,
{
    future: Option<JoinHandle<Result<Tree>>>,
    open_iterators: Vec<std::vec::IntoIter<Node>>,
    inner: std::vec::IntoIter<Node>,
    path: PathBuf,
    be: BE,
}

impl<BE> NodeStreamer<BE>
where
    BE: IndexedBackend + Unpin,
{
    pub fn new(be: BE, id: Id) -> Result<Self> {
        let inner = Tree::from_backend(&be, id)?.nodes.into_iter();
        Ok(Self {
            future: None,
            inner,
            open_iterators: Vec::new(),
            path: PathBuf::new(),
            be,
        })
    }
}

type NodeStreamItem = Result<(PathBuf, Node)>;

// TODO: This is not really parallel at the moment...
impl<BE> Stream for NodeStreamer<BE>
where
    BE: IndexedBackend + Unpin,
{
    type Item = NodeStreamItem;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let slf = self.get_mut();
        if let Some(mut future) = slf.future.as_mut() {
            match Pin::new(&mut future).poll(cx) {
                Poll::Pending => {
                    return Poll::Pending;
                }
                Poll::Ready(tree) => {
                    let old_inner =
                        mem::replace(&mut slf.inner, tree.unwrap().unwrap().nodes.into_iter());
                    slf.open_iterators.push(old_inner);
                    slf.future = None;
                }
            }
        }

        loop {
            match slf.inner.next() {
                Some(node) => {
                    let path = slf.path.join(node.name());
                    if let Some(id) = node.subtree() {
                        slf.path.push(node.name());
                        let be = slf.be.clone();
                        let id = *id;
                        slf.future = Some(spawn(async move { Tree::from_backend(&be, id) }));
                    }

                    return Poll::Ready(Some(Ok((path, node))));
                }
                None => match slf.open_iterators.pop() {
                    Some(it) => {
                        slf.inner = it;
                        slf.path.pop();
                    }
                    None => return Poll::Ready(None),
                },
            }
        }
    }
}

/// TreeStreamerOnce recursively visits all trees and subtrees, but each tree ID only once
pub struct TreeStreamerOnce<BE>
where
    BE: IndexedBackend + Unpin,
{
    futures: FuturesUnordered<JoinHandle<(PathBuf, Result<Tree>, usize)>>,
    visited: HashSet<Id>,
    pending: VecDeque<(PathBuf, Id, usize)>,
    be: BE,
    p: ProgressBar,
    counter: Vec<usize>,
}

impl<BE> TreeStreamerOnce<BE>
where
    BE: IndexedBackend + Unpin,
{
    pub async fn new(be: BE, ids: Vec<Id>, p: ProgressBar) -> Result<Self> {
        p.set_length(ids.len() as u64);
        let counter = vec![0; ids.len()];
        let mut streamer = Self {
            futures: FuturesUnordered::new(),
            visited: HashSet::new(),
            pending: VecDeque::new(),
            be,
            p,
            counter,
        };

        for (count, id) in ids.into_iter().enumerate() {
            if !streamer.add_pending(PathBuf::new(), id, count) {
                streamer.p.inc(1);
            }
        }

        Ok(streamer)
    }

    fn add_pending(&mut self, path: PathBuf, id: Id, count: usize) -> bool {
        if self.visited.insert(id) {
            self.pending.push_back((path, id, count));
            self.counter[count] += 1;
            true
        } else {
            false
        }
    }
}

type TreeStreamItem = Result<(PathBuf, Tree)>;

const MAX_TREE_LOADER: usize = 20;

impl<BE> Stream for TreeStreamerOnce<BE>
where
    BE: IndexedBackend + Unpin,
{
    type Item = TreeStreamItem;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let slf = self.get_mut();

        // fill futures queue if there is space
        while slf.futures.len() < MAX_TREE_LOADER && !slf.pending.is_empty() {
            let (path, id, count) = slf.pending.pop_front().unwrap();
            let be = slf.be.clone();
            slf.futures.push(spawn(
                async move { (path, Tree::from_backend(&be, id), count) },
            ));
        }

        match Pin::new(&mut slf.futures).poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(Ok((path, tree, count)))) => {
                let tree = tree.unwrap();
                for node in tree.nodes() {
                    if let Some(id) = node.subtree() {
                        let mut path = path.clone();
                        path.push(node.name());
                        slf.add_pending(path, *id, count);
                    }
                }
                slf.counter[count] -= 1;
                if slf.counter[count] == 0 {
                    slf.p.inc(1);
                }
                Poll::Ready(Some(Ok((path, tree))))
            }
            Poll::Ready(None) => {
                slf.p.finish();
                Poll::Ready(None)
            }
            Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err(err.into()))),
        }
    }
}
