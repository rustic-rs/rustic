use std::io::Read;

use anyhow::{anyhow, Result};
use indicatif::ProgressBar;

use crate::backend::{DecryptWriteBackend, ReadSourceOpen};
use crate::blob::{BlobType, Node, NodeType, Packer, PackerStats};
use crate::chunker::{ChunkIter, Rabin64};
use crate::crypto::hash;
use crate::index::{IndexedBackend, SharedIndexer};
use crate::repofile::ConfigFile;

use super::{ItemWithParent, ParentResult, TreeItem, TreeType};

#[derive(Clone)]
pub struct FileArchiver<BE: DecryptWriteBackend, I: IndexedBackend> {
    index: I,
    data_packer: Packer<BE>,
    rabin: Rabin64,
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
        let rabin = Rabin64::new_with_polynom(6, poly);
        Ok(Self {
            index,
            data_packer,
            rabin,
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
                } else if let NodeType::File = node.node_type {
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
        let chunks: Vec<_> = ChunkIter::new(r, node.meta.size as usize, self.rabin.clone())
            .map(|chunk| {
                let chunk = chunk?;
                let id = hash(&chunk);
                let size = chunk.len() as u64;

                if !self.index.has_data(&id) {
                    self.data_packer.add(chunk.into(), id)?;
                }
                p.inc(size);
                Ok((id, size))
            })
            .collect::<Result<_>>()?;

        let filesize = chunks.iter().map(|x| x.1).sum();
        let content = chunks.into_iter().map(|x| x.0).collect();

        let mut node = node;
        node.content = Some(content);
        Ok((node, filesize))
    }

    pub fn finalize(self) -> Result<PackerStats> {
        self.data_packer.finalize()
    }
}
