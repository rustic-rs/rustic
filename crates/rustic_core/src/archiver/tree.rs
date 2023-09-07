use std::{ffi::OsString, path::PathBuf};

use crate::{
    backend::{node::Metadata, node::Node, node::NodeType},
    blob::tree::comp_to_osstr,
};

/// `TreeIterator` turns an Iterator yielding items with paths and Nodes into an
/// Iterator which ensures that all subdirectories are visited and closed.
/// The resulting Iterator yielss a `TreeType` which either contains the original
/// item, a new tree to be inserted or a pseudo item which indicates that a tree is finished.
///
/// # Type Parameters
///
/// * `T` - The type of the current item.
/// * `I` - The type of the original Iterator.
pub(crate) struct TreeIterator<T, I> {
    /// The original Iterator.
    iter: I,
    /// The current path.
    path: PathBuf,
    /// The current item.
    item: Option<T>,
}

impl<T, I> TreeIterator<T, I>
where
    I: Iterator<Item = T>,
{
    pub(crate) fn new(mut iter: I) -> Self {
        let item = iter.next();
        Self {
            iter,
            path: PathBuf::new(),
            item,
        }
    }
}

/// `TreeType` is the type returned by the `TreeIterator`.
///
/// It either contains the original item, a new tree to be inserted
/// or a pseudo item which indicates that a tree is finished.
///
/// # Type Parameters
///
/// * `T` - The type of the original item.
/// * `U` - The type of the new tree.
#[derive(Debug)]
pub(crate) enum TreeType<T, U> {
    /// New tree to be inserted.
    NewTree((PathBuf, Node, U)),
    /// A pseudo item which indicates that a tree is finished.
    EndTree,
    /// Original item.
    Other((PathBuf, Node, T)),
}

impl<I, O> Iterator for TreeIterator<(PathBuf, Node, O), I>
where
    I: Iterator<Item = (PathBuf, Node, O)>,
{
    type Item = TreeType<O, OsString>;
    fn next(&mut self) -> Option<Self::Item> {
        match &self.item {
            None => {
                if self.path.pop() {
                    Some(TreeType::EndTree)
                } else {
                    // Check if we still have a path prefix open...
                    match self.path.components().next() {
                        Some(std::path::Component::Prefix(..)) => {
                            self.path = PathBuf::new();
                            Some(TreeType::EndTree)
                        }
                        _ => None,
                    }
                }
            }
            Some((path, node, _)) => {
                match path.strip_prefix(&self.path) {
                    Err(_) => {
                        _ = self.path.pop();
                        Some(TreeType::EndTree)
                    }
                    Ok(missing_dirs) => {
                        for comp in missing_dirs.components() {
                            self.path.push(comp);
                            // process next normal path component - other components are simply ignored
                            if let Some(p) = comp_to_osstr(comp).ok().flatten() {
                                if node.is_dir() && path == &self.path {
                                    let (path, node, _) = self.item.take().unwrap();
                                    self.item = self.iter.next();
                                    let name = node.name();
                                    return Some(TreeType::NewTree((path, node, name)));
                                }
                                let node = Node::new_node(&p, NodeType::Dir, Metadata::default());
                                return Some(TreeType::NewTree((self.path.clone(), node, p)));
                            }
                        }
                        // there wasn't any normal path component to process - return current item
                        let item = self.item.take().unwrap();
                        self.item = self.iter.next();
                        Some(TreeType::Other(item))
                    }
                }
            }
        }
    }
}
