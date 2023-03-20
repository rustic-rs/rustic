use std::io::Read;

use anyhow::{anyhow, Result};
use indicatif::ProgressBar;
use rayon::prelude::*;

use crate::backend::{DecryptWriteBackend, ReadSourceOpen};
use crate::blob::{BlobType, Node, NodeType, Packer, PackerStats};
use crate::chunker::ChunkIter;
use crate::crypto::hash;
use crate::index::{IndexedBackend, SharedIndexer};
use crate::repofile::ConfigFile;

use super::{ItemWithParent, ParentResult, TreeItem, TreeType};

#[derive(Clone)]
pub struct FileArchiver<BE: DecryptWriteBackend, I: IndexedBackend> {
    index: I,
    data_packer: Packer<BE>,
    poly: u64,
}

impl<BE: DecryptWriteBackend, I: IndexedBackend> FileArchiver<BE, I> {
    pub fn new(be: BE, index: I, indexer: SharedIndexer<BE>, config: &ConfigFile) -> Result<Self> {
        let poly = config.poly()?;

        let data_packer = Packer::new(
            be,
            BlobType::Data,
            indexer,
            config,
            index.total_size(BlobType::Data),
        )?;
        Ok(Self {
            index,
            data_packer,
            poly,
        })
    }

    pub fn process<O: ReadSourceOpen>(
        &self,
        item: ItemWithParent<Option<O>>,
        p: ProgressBar,
    ) -> Result<TreeItem> {
        Ok(match item {
            TreeType::NewTree(item) => TreeType::NewTree(item),
            TreeType::EndTree => TreeType::EndTree,
            TreeType::Other((path, node, (open, parent))) => {
                let (node, filesize) = if let ParentResult::Matched(()) = parent {
                    let size = node.meta.size;
                    p.inc(size);
                    (node, size)
                } else if let NodeType::File = node.node_type() {
                    let r = open.ok_or(anyhow!("cannot open file"))?.open()?;
                    self.backup_reader(r, node, p)?
                } else {
                    (node, 0)
                };
                TreeType::Other((path, node, (parent, filesize)))
            }
        })
    }

    pub fn backup_reader(
        &self,
        r: impl Read + Send + 'static,
        node: Node,
        p: ProgressBar,
    ) -> Result<(Node, u64)> {
        let mut chunks: Vec<_> = ChunkIter::new(r, *node.meta().size() as usize, self.poly)
            .enumerate() // see below
            .par_bridge()
            .map(|(num, chunk)| {
                let chunk = chunk?;
                let id = hash(&chunk);
                let size = chunk.len() as u64;

                if !self.index.has_data(&id) {
                    self.data_packer.add(&chunk, &id)?;
                }
                p.inc(size);
                Ok((num, id, size))
            })
            .collect::<Result<_>>()?;

        // As par_bridge doesn't guarantee to keep the order, we sort by the enumeration
        chunks.par_sort_unstable_by_key(|x| x.0);

        let filesize = chunks.iter().map(|x| x.2).sum();
        let content = chunks.into_iter().map(|x| x.1).collect();

        let mut node = node;
        node.set_content(content);
        Ok((node, filesize))
    }

    pub fn finalize(self) -> Result<PackerStats> {
        self.data_packer.finalize()
    }
}
