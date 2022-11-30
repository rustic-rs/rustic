use std::cmp::Ordering;

use anyhow::Result;
use log::warn;

use crate::blob::{Node, Tree};
use crate::id::Id;
use crate::index::IndexedBackend;

pub struct Parent<BE: IndexedBackend> {
    tree: Option<Tree>,
    be: BE,
    node_idx: usize,
    ignore_ctime: bool,
    ignore_inode: bool,
}

pub enum ParentResult<T> {
    Matched(T),
    NotFound,
    NotMatched,
}

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
            be: be.clone(),
            node_idx: 0,
            ignore_ctime,
            ignore_inode,
        }
    }

    pub fn p_node(&mut self, node: &Node) -> Option<&Node> {
        match &self.tree {
            None => None,
            Some(tree) => {
                let name = node.name();
                let p_nodes = tree.nodes();
                loop {
                    match p_nodes.get(self.node_idx) {
                        None => break None,
                        Some(p_node) => match p_node.name().cmp(&name) {
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

    pub fn is_parent(&mut self, node: &Node) -> ParentResult<&Node> {
        // use new variables as the mutable borrow is used later
        let ignore_ctime = self.ignore_ctime;
        let ignore_inode = self.ignore_inode;

        match self.p_node(node) {
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

    pub fn sub_parent(&mut self, node: &Node) -> Result<Self> {
        let tree = match self.p_node(node) {
            Some(p_node) if p_node.node_type() == node.node_type() => match p_node.subtree {
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
            _ => None,
        };
        Ok(Self {
            tree,
            be: self.be.clone(),
            node_idx: 0,
            ignore_ctime: self.ignore_ctime,
            ignore_inode: self.ignore_inode,
        })
    }
}
