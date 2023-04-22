use std::cmp::Ordering;
use std::ffi::{OsStr, OsString};

use anyhow::{anyhow, Result};
use log::warn;

use crate::blob::{Node, Tree};
use crate::id::Id;
use crate::index::IndexedBackend;

use super::TreeType;

pub struct Parent<BE: IndexedBackend> {
    tree: Option<Tree>,
    node_idx: usize,
    stack: Vec<(Option<Tree>, usize)>,
    be: BE,
    ignore_ctime: bool,
    ignore_inode: bool,
}

#[derive(Clone, Debug)]
pub enum ParentResult<T> {
    Matched(T),
    NotFound,
    NotMatched,
}

impl<T> ParentResult<T> {
    fn map<R>(self, f: impl FnOnce(T) -> R) -> ParentResult<R> {
        match self {
            Self::Matched(t) => ParentResult::Matched(f(t)),
            Self::NotFound => ParentResult::NotFound,
            Self::NotMatched => ParentResult::NotMatched,
        }
    }
}

pub type ItemWithParent<O> = TreeType<(O, ParentResult<()>), ParentResult<Id>>;

impl<BE: IndexedBackend> Parent<BE> {
    pub fn new(be: &BE, tree_id: Option<Id>, ignore_ctime: bool, ignore_inode: bool) -> Self {
        // if tree_id is given, try to load tree from backend.
        let tree = match tree_id {
            None => None,
            Some(tree_id) => match Tree::from_backend(be, tree_id) {
                Ok(tree) => Some(tree),
                Err(err) => {
                    warn!("ignoring error when loading parent tree {tree_id}: {err}");
                    None
                }
            },
        };
        Self {
            tree,
            node_idx: 0,
            stack: Vec::new(),
            be: be.clone(),
            ignore_ctime,
            ignore_inode,
        }
    }

    pub fn p_node(&mut self, name: &OsStr) -> Option<&Node> {
        match &self.tree {
            None => None,
            Some(tree) => {
                let p_nodes = &tree.nodes;
                loop {
                    match p_nodes.get(self.node_idx) {
                        None => break None,
                        Some(p_node) => match p_node.name().as_os_str().cmp(name) {
                            Ordering::Less => self.node_idx += 1,
                            Ordering::Equal => {
                                break Some(p_node);
                            }
                            Ordering::Greater => {
                                break None;
                            }
                        },
                    }
                }
            }
        }
    }

    pub fn is_parent(&mut self, node: &Node, name: &OsStr) -> ParentResult<&Node> {
        // use new variables as the mutable borrow is used later
        let ignore_ctime = self.ignore_ctime;
        let ignore_inode = self.ignore_inode;

        match self.p_node(name) {
            None => ParentResult::NotFound,
            Some(p_node) => {
                if p_node.node_type == node.node_type
                    && p_node.meta.size == node.meta.size
                    && p_node.meta.mtime == node.meta.mtime
                    && (ignore_ctime || p_node.meta.ctime == node.meta.ctime)
                    && (ignore_inode
                        || p_node.meta.inode == 0
                        || p_node.meta.inode == node.meta.inode)
                {
                    ParentResult::Matched(p_node)
                } else {
                    ParentResult::NotMatched
                }
            }
        }
    }

    pub fn set_dir(&mut self, name: &OsStr) -> Result<()> {
        let tree = match self.p_node(name) {
            Some(p_node) => match p_node.subtree {
                Some(tree_id) => match Tree::from_backend(&self.be, tree_id) {
                    Ok(tree) => Some(tree),
                    Err(err) => {
                        warn!("ignoring error when loading parent tree {tree_id}: {err}");
                        None
                    }
                },
                None => {
                    warn!("ignoring parent node {}: is no tree!", p_node.name);
                    None
                }
            },
            None => None,
        };
        self.stack.push((self.tree.take(), self.node_idx));
        self.tree = tree;
        self.node_idx = 0;
        Ok(())
    }

    pub fn finish_dir(&mut self) -> Result<()> {
        let (tree, node_idx) = self
            .stack
            .pop()
            .ok_or_else(|| anyhow!("tree stack empty??"))?;

        self.tree = tree;
        self.node_idx = node_idx;

        Ok(())
    }

    pub fn process<O>(&mut self, item: TreeType<O, OsString>) -> Result<ItemWithParent<O>> {
        let result = match item {
            TreeType::NewTree((path, node, tree)) => {
                let parent_result = self
                    .is_parent(&node, &tree)
                    .map(|node| node.subtree.unwrap());
                self.set_dir(&tree)?;
                TreeType::NewTree((path, node, parent_result))
            }
            TreeType::EndTree => {
                self.finish_dir()?;
                TreeType::EndTree
            }
            TreeType::Other((path, mut node, open)) => {
                let be = self.be.clone();
                let parent = self.is_parent(&node, &node.name());
                let parent = match parent {
                    ParentResult::Matched(p_node) => {
                        if p_node.content.iter().flatten().all(|id| be.has_data(id)) {
                            node.content = Some(p_node.content.iter().flatten().copied().collect());
                            ParentResult::Matched(())
                        } else {
                            warn!(
                            "missing blobs in index for unchanged file {path:?}; re-reading file",
                        );
                            ParentResult::NotFound
                        }
                    }
                    parent_result => parent_result.map(|_| ()),
                };
                TreeType::Other((path, node, (open, parent)))
            }
        };
        Ok(result)
    }
}
