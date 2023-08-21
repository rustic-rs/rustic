use std::{
    cmp::Ordering,
    ffi::{OsStr, OsString},
};

use log::warn;

use crate::{
    archiver::tree::TreeType, backend::node::Node, blob::tree::Tree, error::ArchiverErrorKind,
    id::Id, index::IndexedBackend, RusticResult,
};

#[derive(Debug)]
pub struct Parent {
    tree_id: Option<Id>,
    tree: Option<Tree>,
    node_idx: usize,
    stack: Vec<(Option<Tree>, usize)>,
    ignore_ctime: bool,
    ignore_inode: bool,
}

#[derive(Clone, Debug)]
pub(crate) enum ParentResult<T> {
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

pub(crate) type ItemWithParent<O> = TreeType<(O, ParentResult<()>), ParentResult<Id>>;

impl Parent {
    pub(crate) fn new<BE: IndexedBackend>(
        be: &BE,
        tree_id: Option<Id>,
        ignore_ctime: bool,
        ignore_inode: bool,
    ) -> Self {
        // if tree_id is given, try to load tree from backend.
        let tree = tree_id.and_then(|tree_id| match Tree::from_backend(be, tree_id) {
            Ok(tree) => Some(tree),
            Err(err) => {
                warn!("ignoring error when loading parent tree {tree_id}: {err}");
                None
            }
        });
        Self {
            tree_id,
            tree,
            node_idx: 0,
            stack: Vec::new(),
            ignore_ctime,
            ignore_inode,
        }
    }

    fn p_node(&mut self, name: &OsStr) -> Option<&Node> {
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

    fn is_parent(&mut self, node: &Node, name: &OsStr) -> ParentResult<&Node> {
        // use new variables as the mutable borrow is used later
        let ignore_ctime = self.ignore_ctime;
        let ignore_inode = self.ignore_inode;

        self.p_node(name).map_or(ParentResult::NotFound, |p_node| {
            if p_node.node_type == node.node_type
                && p_node.meta.size == node.meta.size
                && p_node.meta.mtime == node.meta.mtime
                && (ignore_ctime || p_node.meta.ctime == node.meta.ctime)
                && (ignore_inode || p_node.meta.inode == 0 || p_node.meta.inode == node.meta.inode)
            {
                ParentResult::Matched(p_node)
            } else {
                ParentResult::NotMatched
            }
        })
    }

    fn set_dir<BE: IndexedBackend>(&mut self, be: &BE, name: &OsStr) {
        let tree = self.p_node(name).and_then(|p_node| {
            p_node.subtree.map_or_else(
                || {
                    warn!("ignoring parent node {}: is no tree!", p_node.name);
                    None
                },
                |tree_id| match Tree::from_backend(be, tree_id) {
                    Ok(tree) => Some(tree),
                    Err(err) => {
                        warn!("ignoring error when loading parent tree {tree_id}: {err}");
                        None
                    }
                },
            )
        });
        self.stack.push((self.tree.take(), self.node_idx));
        self.tree = tree;
        self.node_idx = 0;
    }

    fn finish_dir(&mut self) -> RusticResult<()> {
        let (tree, node_idx) = self
            .stack
            .pop()
            .ok_or_else(|| ArchiverErrorKind::TreeStackEmpty)?;

        self.tree = tree;
        self.node_idx = node_idx;

        Ok(())
    }

    pub(crate) fn tree_id(&self) -> Option<Id> {
        self.tree_id
    }

    pub(crate) fn process<BE: IndexedBackend, O>(
        &mut self,
        be: &BE,
        item: TreeType<O, OsString>,
    ) -> RusticResult<ItemWithParent<O>> {
        let result = match item {
            TreeType::NewTree((path, node, tree)) => {
                let parent_result = self
                    .is_parent(&node, &tree)
                    .map(|node| node.subtree.unwrap());
                self.set_dir(be, &tree);
                TreeType::NewTree((path, node, parent_result))
            }
            TreeType::EndTree => {
                self.finish_dir()?;
                TreeType::EndTree
            }
            TreeType::Other((path, mut node, open)) => {
                let be = be.clone();
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
