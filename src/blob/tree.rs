use std::collections::HashSet;
use std::mem;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use derive_getters::Getters;
use serde::{Deserialize, Serialize};

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

    pub fn from_backend(be: &impl IndexedBackend, id: &Id) -> Result<Self> {
        let data = be
            .get_tree(id)
            .ok_or(anyhow!("blob not found in index"))?
            .read_data(be.be())?;

        Ok(serde_json::from_slice(&data)?)
    }
}

type TreeIterItem = Result<(PathBuf, Node)>;

/// tree_iterator creates an Iterator over the trees given by ids using the backend be and the index
/// index
pub fn tree_iterator(
    be: &impl IndexedBackend,
    ids: Vec<Id>,
) -> Result<impl Iterator<Item = TreeIterItem> + '_> {
    TreeIterator::new(|i| Ok(Tree::from_backend(be, i)?.nodes.into_iter()), ids)
}

/// tree_iterator_once creates an Iterator over the trees given by ids using the backend be and the index
/// index where each node is only visited once
pub fn tree_iterator_once(
    be: &impl IndexedBackend,
    ids: Vec<Id>,
) -> Result<impl Iterator<Item = TreeIterItem> + '_> {
    let mut visited = HashSet::new();
    TreeIterator::new(
        move |i| {
            if visited.insert(*i) {
                Ok(Tree::from_backend(be, i)?.nodes.into_iter())
            } else {
                Ok(Vec::new().into_iter())
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
    F: FnMut(&Id) -> Result<IT>,
{
    open_iterators: Vec<IT>,
    inner: IT,
    path: PathBuf,
    getter: F,
}

impl<IT, F> TreeIterator<IT, F>
where
    IT: Iterator<Item = Node>,
    F: FnMut(&Id) -> Result<IT>,
{
    fn new(mut getter: F, ids: Vec<Id>) -> Result<Self> {
        // TODO: empty ids vector will panic here!
        let mut iters: Vec<_> = ids.iter().map(&mut getter).collect::<Result<_>>()?;
        iters.rotate_right(1);
        Ok(Self {
            inner: iters.pop().unwrap(),
            open_iterators: iters,
            path: PathBuf::new(),
            getter,
        })
    }
}

impl<IT, F> Iterator for TreeIterator<IT, F>
where
    IT: Iterator<Item = Node>,
    F: FnMut(&Id) -> Result<IT>,
{
    type Item = Result<(PathBuf, Node)>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                Some(node) => {
                    let path = self.path.join(node.name());
                    if let Some(id) = node.subtree() {
                        let old_inner = mem::replace(&mut self.inner, (self.getter)(id).unwrap());
                        self.open_iterators.push(old_inner);
                        self.path.push(node.name());
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
