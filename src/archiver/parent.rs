use anyhow::Result;

use crate::blob::{Node, Tree};
use crate::id::Id;
use crate::index::IndexedBackend;

pub struct Parent<BE: IndexedBackend> {
    tree: Option<Tree>,
    be: BE,
}

pub enum ParentResult<T> {
    Matched(T),
    NotFound,
    NotMatched,
}

impl<BE: IndexedBackend> Parent<BE> {
    pub async fn new(be: &BE, tree_id: Option<Id>) -> Self {
        // if tree_id is given, load tre from backend. Turn errors into None.
        // TODO: print warning when loading failed
        let tree = match tree_id {
            None => None,
            Some(id) => Tree::from_backend(be, id).await.ok(),
        };
        Self {
            tree,
            be: be.clone(),
        }
    }

    pub fn p_node(&self, node: &Node) -> Option<&Node> {
        match &self.tree {
            None => None,
            Some(tree) => tree
                .nodes()
                .iter()
                .find(|p_node| p_node.name() == node.name()),
        }
    }

    pub fn is_parent(&self, node: &Node) -> ParentResult<&Node> {
        match self.p_node(node) {
            None => ParentResult::NotFound,
            Some(p_node) => {
                if p_node.node_type() == node.node_type()
                    && p_node.meta().ctime() == node.meta().ctime()
                    && p_node.meta().inode() > &0
                    && p_node.meta().inode() == node.meta().inode()
                {
                    ParentResult::Matched(p_node)
                } else {
                    ParentResult::NotMatched
                }
            }
        }
    }

    pub async fn sub_parent(&self, node: &Node) -> Result<Self> {
        let tree = match self.p_node(node) {
            None => None,
            Some(p_node) => {
                if p_node.node_type() == node.node_type() {
                    // TODO: print warning when loading failed
                    Some(
                        Tree::from_backend(&self.be, p_node.subtree().unwrap())
                            .await
                            .ok(),
                    )
                    .flatten()
                } else {
                    None
                }
            }
        };
        Ok(Self {
            tree,
            be: self.be.clone(),
        })
    }
}
