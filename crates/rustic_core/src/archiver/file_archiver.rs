use std::io::Read;

use indicatif::ProgressBar;

use crate::{
    archiver::{
        parent::{ItemWithParent, ParentResult},
        tree::TreeType,
        tree_archiver::TreeItem,
    },
    backend::{
        decrypt::DecryptWriteBackend,
        node::{Node, NodeType},
        ReadSourceOpen,
    },
    blob::{
        packer::{Packer, PackerStats},
        BlobType,
    },
    cdc::rolling_hash::Rabin64,
    chunker::ChunkIter,
    crypto::hasher::hash,
    error::ArchiverErrorKind,
    index::{indexer::SharedIndexer, IndexedBackend},
    repofile::configfile::ConfigFile,
    RusticResult,
};

#[derive(Clone)]
pub(crate) struct FileArchiver<BE: DecryptWriteBackend, I: IndexedBackend> {
    index: I,
    data_packer: Packer<BE>,
    rabin: Rabin64,
}

impl<BE: DecryptWriteBackend, I: IndexedBackend> FileArchiver<BE, I> {
    pub(crate) fn new(
        be: BE,
        index: I,
        indexer: SharedIndexer<BE>,
        config: &ConfigFile,
    ) -> RusticResult<Self> {
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

    pub(crate) fn process<O: ReadSourceOpen>(
        &self,
        item: ItemWithParent<Option<O>>,
        p: &ProgressBar,
    ) -> RusticResult<TreeItem> {
        Ok(match item {
            TreeType::NewTree(item) => TreeType::NewTree(item),
            TreeType::EndTree => TreeType::EndTree,
            TreeType::Other((path, node, (open, parent))) => {
                let (node, filesize) = if matches!(parent, ParentResult::Matched(())) {
                    let size = node.meta.size;
                    p.inc(size);
                    (node, size)
                } else if node.node_type == NodeType::File {
                    let r = open
                        .ok_or(ArchiverErrorKind::UnpackingTreeTypeOptionalFailed)?
                        .open()?;
                    self.backup_reader(r, node, p)?
                } else {
                    (node, 0)
                };
                TreeType::Other((path, node, (parent, filesize)))
            }
        })
    }

    fn backup_reader(
        &self,
        r: impl Read + Send + 'static,
        node: Node,
        p: &ProgressBar,
    ) -> RusticResult<(Node, u64)> {
        let chunks: Vec<_> = ChunkIter::new(
            r,
            usize::try_from(node.meta.size)
                .map_err(ArchiverErrorKind::ConversionFromU64ToUsizeFailed)?,
            self.rabin.clone(),
        )
        .map(|chunk| {
            let chunk = chunk.map_err(ArchiverErrorKind::FromStdIo)?;
            let id = hash(&chunk);
            let size = chunk.len() as u64;

            if !self.index.has_data(&id) {
                self.data_packer.add(chunk.into(), id)?;
            }
            p.inc(size);
            Ok((id, size))
        })
        .collect::<RusticResult<_>>()?;

        let filesize = chunks.iter().map(|x| x.1).sum();
        let content = chunks.into_iter().map(|x| x.0).collect();

        let mut node = node;
        node.content = Some(content);
        Ok((node, filesize))
    }

    pub(crate) fn finalize(self) -> RusticResult<PackerStats> {
        self.data_packer.finalize()
    }
}
