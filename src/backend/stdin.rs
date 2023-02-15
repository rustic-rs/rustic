use std::io::{stdin, Stdin};
use std::path::Path;

use anyhow::Result;

use super::{node::Metadata, node::NodeType, Node, ReadSource};
use super::{ReadSourceEntry, ReadSourceOpen};

pub struct StdinSource {
    finished: bool,
}

impl StdinSource {
    pub fn new() -> Result<Self> {
        Ok(Self { finished: false })
    }
}

pub struct OpenStdin();

impl ReadSourceOpen for OpenStdin {
    type Reader = Stdin;

    fn open(self) -> Result<Self::Reader> {
        Ok(stdin())
    }
}

impl ReadSource for StdinSource {
    type Open = OpenStdin;
    type Iter = Self;

    fn size(&self) -> Result<Option<u64>> {
        Ok(None)
    }

    fn entries(self) -> Self::Iter {
        self
    }
}

impl Iterator for StdinSource {
    type Item = Result<ReadSourceEntry<OpenStdin>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        self.finished = true;

        Some(Ok(ReadSourceEntry {
            path: Path::new("stdin").to_path_buf(),
            node: Node::new(
                "stdin".to_string(),
                NodeType::File,
                Metadata::default(),
                None,
                None,
            ),
            open: Some(OpenStdin()),
        }))
    }
}
