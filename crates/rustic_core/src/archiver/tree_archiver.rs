use std::path::{Path, PathBuf};

use bytesize::ByteSize;
use log::{debug, trace};

use crate::{
    archiver::{parent::ParentResult, tree::TreeType},
    backend::{decrypt::DecryptWriteBackend, node::Node},
    blob::{packer::Packer, tree::Tree, BlobType},
    error::ArchiverErrorKind,
    error::RusticResult,
    id::Id,
    index::{indexer::SharedIndexer, IndexedBackend},
    repofile::{configfile::ConfigFile, snapshotfile::SnapshotSummary},
};

pub(crate) type TreeItem = TreeType<(ParentResult<()>, u64), ParentResult<Id>>;

/// The `TreeArchiver` is responsible for archiving trees.
///
/// # Type Parameters
///
/// * `BE` - The backend type.
/// * `I` - The index to read from.
///
// TODO: Add documentation
pub(crate) struct TreeArchiver<BE: DecryptWriteBackend, I: IndexedBackend> {
    /// The current tree.
    tree: Tree,
    /// The stack of trees.
    stack: Vec<(PathBuf, Node, ParentResult<Id>, Tree)>,
    /// The index to read from.
    index: I,
    /// The packer to write to.
    tree_packer: Packer<BE>,
    /// The summary of the snapshot.
    summary: SnapshotSummary,
}

impl<BE: DecryptWriteBackend, I: IndexedBackend> TreeArchiver<BE, I> {
    /// Creates a new `TreeArchiver`.
    ///
    /// # Type Parameters
    ///
    /// * `BE` - The backend type.
    /// * `I` - The index to read from.
    ///
    /// # Arguments
    ///
    /// * `be` - The backend to write to.
    /// * `index` - The index to read from.
    /// * `indexer` - The indexer to write to.
    /// * `config` - The config file.
    /// * `summary` - The summary of the snapshot.
    ///
    /// # Errors
    ///
    /// * [`PackerErrorKind::ZstdError`] - If the zstd compression level is invalid.
    /// * [`PackerErrorKind::SendingCrossbeamMessageFailed`] - If sending the message to the raw packer fails.
    /// * [`PackerErrorKind::IntConversionFailed`] - If converting the data length to u64 fails
    pub(crate) fn new(
        be: BE,
        index: I,
        indexer: SharedIndexer<BE>,
        config: &ConfigFile,
        summary: SnapshotSummary,
    ) -> RusticResult<Self> {
        let tree_packer = Packer::new(
            be,
            BlobType::Tree,
            indexer,
            config,
            index.total_size(BlobType::Tree),
        )?;
        Ok(Self {
            tree: Tree::new(),
            stack: Vec::new(),
            index,
            tree_packer,
            summary,
        })
    }

    /// Adds the given item to the tree.
    ///
    /// # Arguments
    ///
    /// * `item` - The item to add.
    ///
    /// # Errors
    ///
    /// * [`ArchiverErrorKind::TreeStackEmpty`] - If the tree stack is empty.
    // TODO: Add more errors!
    pub(crate) fn add(&mut self, item: TreeItem) -> RusticResult<()> {
        match item {
            TreeType::NewTree((path, node, parent)) => {
                trace!("entering {path:?}");
                // save current tree to the stack and start with an empty tree
                let tree = std::mem::replace(&mut self.tree, Tree::new());
                self.stack.push((path, node, parent, tree));
            }
            TreeType::EndTree => {
                let (path, mut node, parent, tree) = self
                    .stack
                    .pop()
                    .ok_or_else(|| ArchiverErrorKind::TreeStackEmpty)?;

                // save tree
                trace!("finishing {path:?}");
                let id = self.backup_tree(&path, &parent)?;
                node.subtree = Some(id);

                // go back to parent dir
                self.tree = tree;
                self.tree.add(node);
            }
            TreeType::Other((path, node, (parent, size))) => {
                self.add_file(&path, node, &parent, size);
            }
        }
        Ok(())
    }

    /// Adds the given file to the tree.
    ///
    /// # Arguments
    ///
    /// * `path` - The path of the file.
    /// * `node` - The node of the file.
    /// * `parent` - The parent result of the file.
    fn add_file(&mut self, path: &Path, node: Node, parent: &ParentResult<()>, size: u64) {
        let filename = path.join(node.name());
        match parent {
            ParentResult::Matched(_) => {
                debug!("unchanged file: {:?}", filename);
                self.summary.files_unmodified += 1;
            }
            ParentResult::NotMatched => {
                debug!("changed   file: {:?}", filename);
                self.summary.files_changed += 1;
            }
            ParentResult::NotFound => {
                debug!("new       file: {:?}", filename);
                self.summary.files_new += 1;
            }
        }
        self.summary.total_files_processed += 1;
        self.summary.total_bytes_processed += size;
        self.tree.add(node);
    }

    /// Backups the current tree.
    ///
    /// # Arguments
    ///
    /// * `path` - The path of the tree.
    /// * `parent` - The parent result of the tree.
    ///
    /// # Errors
    ///
    /// * [`PackerErrorKind::SendingCrossbeamMessageFailed`] - If sending the message to the raw packer fails.
    ///
    /// # Returns
    ///
    /// The id of the tree.
    fn backup_tree(&mut self, path: &Path, parent: &ParentResult<Id>) -> RusticResult<Id> {
        let (chunk, id) = self.tree.serialize()?;
        let dirsize = chunk.len() as u64;
        let dirsize_bytes = ByteSize(dirsize).to_string_as(true);

        self.summary.total_dirs_processed += 1;
        self.summary.total_dirsize_processed += dirsize;
        match parent {
            ParentResult::Matched(p_id) if id == *p_id => {
                debug!("unchanged tree: {:?}", path);
                self.summary.dirs_unmodified += 1;
                return Ok(id);
            }
            ParentResult::NotFound => {
                debug!("new       tree: {:?} {}", path, dirsize_bytes);
                self.summary.dirs_new += 1;
            }
            _ => {
                // "Matched" trees where the subtree id does not match or unmatched trees
                debug!("changed   tree: {:?} {}", path, dirsize_bytes);
                self.summary.dirs_changed += 1;
            }
        }

        if !self.index.has_tree(&id) {
            self.tree_packer.add(chunk.into(), id)?;
        }
        Ok(id)
    }

    /// Finalizes the tree archiver.
    ///
    /// # Arguments
    ///
    /// * `parent_tree` - The parent tree.
    ///
    /// # Errors
    ///
    /// * [`PackerErrorKind::SendingCrossbeamMessageFailed`] - If sending the message to the raw packer fails.
    ///
    /// # Returns
    ///
    /// A tuple containing the id of the tree and the summary of the snapshot.
    ///
    /// # Panics
    ///
    /// If the channel of the tree packer is not dropped.
    pub(crate) fn finalize(
        mut self,
        parent_tree: Option<Id>,
    ) -> RusticResult<(Id, SnapshotSummary)> {
        let parent = parent_tree.map_or(ParentResult::NotFound, ParentResult::Matched);
        let id = self.backup_tree(&PathBuf::new(), &parent)?;
        let stats = self.tree_packer.finalize()?;
        stats.apply(&mut self.summary, BlobType::Tree);

        Ok((id, self.summary))
    }
}
