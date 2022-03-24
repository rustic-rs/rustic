use std::collections::HashSet;
use std::mem;
use std::path::PathBuf;
use std::pin::Pin;

use anyhow::{anyhow, Result};
use derive_getters::Getters;
use futures::{
    task::{Context, Poll},
    Future, Stream,
};
use serde::{Deserialize, Serialize};
use tokio::{spawn, task::JoinHandle};

use crate::id::Id;
use crate::index::IndexedBackend;

use super::Node;

#[derive(Clone, Debug, Serialize, Deserialize, Getters)]
pub struct Tree {
    nodes: Vec<Node>,
}

impl Tree {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    pub fn add(&mut self, node: Node) {
        self.nodes.push(node)
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(&self)?)
    }

    pub async fn from_backend(be: &impl IndexedBackend, id: Id) -> Result<Self> {
        let data = be
            .get_tree(&id)
            .ok_or(anyhow!("blob not found in index"))?
            .read_data(be.be())
            .await?;

        Ok(serde_json::from_slice(&data)?)
    }
}

/// TreeIterator is a recursive iterator over a Tree, i.e. it recursively iterates over
/// a full tree visiting subtrees
pub struct TreeStreamer<BE>
where
    BE: IndexedBackend + Unpin,
{
    future: Option<JoinHandle<Result<Tree>>>,
    visited: HashSet<Id>,
    only_once: bool,
    open_iterators: Vec<std::vec::IntoIter<Node>>,
    inner: std::vec::IntoIter<Node>,
    path: PathBuf,
    be: BE,
}

impl<BE> TreeStreamer<BE>
where
    BE: IndexedBackend + Unpin,
{
    pub async fn new(be: BE, ids: Vec<Id>, only_once: bool) -> Result<Self> {
        // TODO: empty ids vector will panic here!
        let mut iters = Vec::new();
        for id in ids {
            iters.push(Tree::from_backend(&be, id).await?.nodes.into_iter());
        }
        iters.rotate_right(1);
        Ok(Self {
            future: None,
            visited: HashSet::new(),
            only_once,
            inner: iters.pop().unwrap(),
            open_iterators: iters,
            path: PathBuf::new(),
            be: be.clone(),
        })
    }
}

type TreeStreamItem = Result<(PathBuf, Node)>;

impl<BE> Stream for TreeStreamer<BE>
where
    BE: IndexedBackend + Unpin,
{
    type Item = TreeStreamItem;

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
                        if !slf.only_once || !slf.visited.contains(id) {
                            if slf.only_once {
                                slf.visited.insert(*id);
                            }
                            slf.path.push(node.name());
                            let be = slf.be.clone();
                            let id = id.clone();
                            slf.future =
                                Some(spawn(async move { Tree::from_backend(&be, id).await }));
                        }
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
