use std::{io::stdin, path::PathBuf};

use crate::{
    backend::{
        node::Metadata, node::Node, node::NodeType, ReadSource, ReadSourceEntry, ReadSourceOpen,
    },
    RusticResult,
};

#[derive(Debug)]
pub struct StdinSource {
    finished: bool,
    path: PathBuf,
}

impl StdinSource {
    pub const fn new(path: PathBuf) -> RusticResult<Self> {
        Ok(Self {
            finished: false,
            path,
        })
    }
}

#[derive(Debug, Copy, Clone)]
pub struct OpenStdin();

impl ReadSourceOpen for OpenStdin {
    type Reader = std::io::Stdin;

    fn open(self) -> RusticResult<Self::Reader> {
        Ok(stdin())
    }
}

impl ReadSource for StdinSource {
    type Open = OpenStdin;
    type Iter = Self;

    fn size(&self) -> RusticResult<Option<u64>> {
        Ok(None)
    }

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
