use crate::blob::{Node, Tree};
use crate::id::Id;
use crate::index::IndexedBackend;

pub struct Parent<BE: IndexedBackend> {
    tree: Option<Tree>,
    be: BE,
}

impl<BE: IndexedBackend> Parent<BE> {
    pub fn new(be: &BE, tree_id: Option<&Id>) -> Self {
        let tree = tree_id.map(|id| Tree::from_backend(be, id).unwrap());
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

    pub fn is_parent(&self, node: &Node) -> Option<&Node> {
        match self.p_node(node) {
            None => None,
            Some(p_node) => {
                if p_node.node_type() == node.node_type()
                    && p_node.meta().ctime() == node.meta().ctime()
                    && p_node.meta().inode() > &0
                    && p_node.meta().inode() == node.meta().inode()
                    && p_node.content().iter().all(|id| self.be.has(id))
                {
                    Some(p_node)
                } else {
                    None
                }
            }
        }
    }

    pub fn sub_parent(&self, node: &Node) -> Self {
        let tree = match self.p_node(node) {
            None => None,
            Some(p_node) if p_node.node_type() == node.node_type() => {
                Some(Tree::from_backend(&self.be, &p_node.subtree().unwrap()).unwrap())
            }
            Some(..) => None,
        };
        Self {
            tree,
            be: self.be.clone(),
        }
    }
}
