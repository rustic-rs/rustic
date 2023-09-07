use std::{io::stdin, path::PathBuf};

use crate::{
    backend::{
        node::Metadata, node::Node, node::NodeType, ReadSource, ReadSourceEntry, ReadSourceOpen,
    },
    error::RusticResult,
};

/// The `StdinSource` is a `ReadSource` for stdin.
#[derive(Debug)]
pub struct StdinSource {
    /// Whether we have already yielded the stdin entry.
    finished: bool,
    /// The path of the stdin entry.
    path: PathBuf,
}

impl StdinSource {
    /// Creates a new `StdinSource`.
    pub const fn new(path: PathBuf) -> RusticResult<Self> {
        Ok(Self {
            finished: false,
            path,
        })
    }
}

/// The `OpenStdin` is a `ReadSourceOpen` for stdin.
#[derive(Debug, Copy, Clone)]
pub struct OpenStdin();

impl ReadSourceOpen for OpenStdin {
    /// The reader type.
    type Reader = std::io::Stdin;

    /// Opens stdin.
    fn open(self) -> RusticResult<Self::Reader> {
        Ok(stdin())
    }
}

impl ReadSource for StdinSource {
    /// The open type.
    type Open = OpenStdin;
    /// The iterator type.
    type Iter = Self;

    /// Returns the size of the source.
    fn size(&self) -> RusticResult<Option<u64>> {
        Ok(None)
    }

    /// Returns an iterator over the source.
    fn entries(self) -> Self::Iter {
        self
    }
}

impl Iterator for StdinSource {
    type Item = RusticResult<ReadSourceEntry<OpenStdin>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        self.finished = true;

        Some(Ok(ReadSourceEntry {
            path: self.path.clone(),
            node: Node::new_node(
                self.path.file_name().unwrap(),
                NodeType::File,
                Metadata::default(),
            ),
            open: Some(OpenStdin()),
        }))
    }
}
