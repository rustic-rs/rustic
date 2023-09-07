use std::{
    cmp::Ordering,
    ffi::{OsStr, OsString},
};

use log::warn;

use crate::{
    archiver::tree::TreeType, backend::node::Node, blob::tree::Tree, error::ArchiverErrorKind,
    error::RusticResult, id::Id, index::IndexedBackend,
};

/// The `ItemWithParent` is a `TreeType` wrapping the result of a parent search and a type `O`.
///
/// # Type Parameters
///
/// * `O` - The type of the `TreeType`.
pub(crate) type ItemWithParent<O> = TreeType<(O, ParentResult<()>), ParentResult<Id>>;

/// The `Parent` is responsible for finding the parent tree of a given tree.
#[derive(Debug)]
pub struct Parent {
    /// The tree id of the parent tree.
    tree_id: Option<Id>,
    /// The parent tree.
    tree: Option<Tree>,
    /// The current node index.
    node_idx: usize,
    /// The stack of parent trees.
    stack: Vec<(Option<Tree>, usize)>,
    /// Ignore ctime when comparing nodes.
    ignore_ctime: bool,
    /// Ignore inode number when comparing nodes.
    ignore_inode: bool,
}

/// The result of a parent search.
///
/// # Type Parameters
///
/// * `T` - The type of the matched parent.
#[derive(Clone, Debug)]
pub(crate) enum ParentResult<T> {
    /// The parent was found and matches.
    Matched(T),
    /// The parent was not found.
    NotFound,
    /// The parent was found but doesn't match.
    NotMatched,
}

impl<T> ParentResult<T> {
    /// Maps a `ParentResult<T>` to a `ParentResult<R>` by applying a function to a contained value.
    ///
    /// # Type Parameters
    ///
    /// * `R` - The type of the returned `ParentResult`.
    ///
    /// # Arguments
    ///
    /// * `f` - The function to apply.
    ///
    /// # Returns
    ///
    /// A `ParentResult<R>` with the result of the function for each `ParentResult<T>`.
    fn map<R>(self, f: impl FnOnce(T) -> R) -> ParentResult<R> {
        match self {
            Self::Matched(t) => ParentResult::Matched(f(t)),
            Self::NotFound => ParentResult::NotFound,
            Self::NotMatched => ParentResult::NotMatched,
        }
    }
}

impl Parent {
    /// Creates a new `Parent`.
    ///
    /// # Type Parameters
    ///
    /// * `BE` - The type of the backend.
    ///
    /// # Arguments
    ///
    /// * `be` - The backend to read from.
    /// * `tree_id` - The tree id of the parent tree.
    /// * `ignore_ctime` - Ignore ctime when comparing nodes.
    /// * `ignore_inode` - Ignore inode number when comparing nodes.
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

    /// Returns the parent node with the given name.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the parent node.
    ///
    /// # Returns
    ///
    /// The parent node with the given name, or `None` if the parent node is not found.
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

    /// Returns whether the given node is the parent of the given tree.
    ///
    /// # Arguments
    ///
    /// * `node` - The node to check.
    /// * `name` - The name of the tree.
    ///
    /// # Returns
    ///
    /// Whether the given node is the parent of the given tree.
    ///
    /// # Note
    ///
    /// TODO: This function does not check whether the given node is a directory.
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

    // TODO: add documentation!
    ///
    /// # Type Parameters
    ///
    /// * `BE` - The type of the backend.
    ///
    /// # Arguments
    ///
    /// * `be` - The backend to read from.
    /// * `name` - The name of the parent node.
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

    // TODO: add documentation!
    ///
    /// # Errors
    ///
    /// * [`ArchiverErrorKind::TreeStackEmpty`] - If the tree stack is empty.
    fn finish_dir(&mut self) -> RusticResult<()> {
        let (tree, node_idx) = self
            .stack
            .pop()
            .ok_or_else(|| ArchiverErrorKind::TreeStackEmpty)?;

        self.tree = tree;
        self.node_idx = node_idx;

        Ok(())
    }

    // TODO: add documentation!
    pub(crate) fn tree_id(&self) -> Option<Id> {
        self.tree_id
    }

    // TODO: add documentation!
    ///
    /// # Type Parameters
    ///
    /// * `BE` - The type of the backend.
    /// * `O` - The type of the tree item.
    ///
    /// # Arguments
    ///
    /// * `be` - The backend to read from.
    /// * `item` - The item to process.
    ///
    /// # Errors
    ///
    /// * [`ArchiverErrorKind::TreeStackEmpty`] - If the tree stack is empty.
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
