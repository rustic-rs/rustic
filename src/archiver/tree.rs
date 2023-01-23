use std::ffi::OsString;
use std::path::{Component, PathBuf};

use crate::blob::{Metadata, Node, NodeType};

pub struct TreeIterator<T, I> {
    iter: I,
    path: PathBuf,
    item: Option<T>,
}

impl<T, I> TreeIterator<T, I>
where
    I: Iterator<Item = T>,
{
    pub fn new(mut iter: I) -> Self {
        let item = iter.next();
        Self {
            iter,
            path: PathBuf::new(),
            item,
        }
    }
}

#[derive(Debug)]
pub enum TreeType<T, U> {
    NewTree(U),
    EndTree,
    Other(T),
}

impl<I> Iterator for TreeIterator<(PathBuf, PathBuf, Node), I>
where
    I: Iterator<Item = (PathBuf, PathBuf, Node)>,
{
    type Item = TreeType<(PathBuf, PathBuf, Node), (PathBuf, Node, OsString)>;
    fn next(&mut self) -> Option<Self::Item> {
        match &self.item {
            None => {
                if self.path.pop() {
                    Some(TreeType::EndTree)
                } else {
                    None
                }
            }
            Some((path, _, node)) => {
                match path.strip_prefix(&self.path) {
                    Err(_) => {
                        self.path.pop();
                        Some(TreeType::EndTree)
                    }
                    Ok(missing_dirs) => {
                        for comp in missing_dirs.components() {
                            self.path.push(comp);
                            // process next normal path component - other components are simply ignored
                            if let Component::Normal(p) = comp {
                                if node.is_dir() && path == &self.path {
                                    let (path, _, node) = self.item.take().unwrap();
                                    self.item = self.iter.next();
                                    let name = node.name();
                                    return Some(TreeType::NewTree((path, node, name)));
                                } else {
                                    let node =
                                        Node::new_node(p, NodeType::Dir, Metadata::default());
                                    return Some(TreeType::NewTree((
                                        self.path.clone(),
                                        node,
                                        p.to_os_string(),
                                    )));
                                }
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
