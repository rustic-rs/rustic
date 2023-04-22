use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use bytesize::ByteSize;
use log::*;

use crate::backend::DecryptWriteBackend;
use crate::blob::{BlobType, Node, Packer, Tree};
use crate::id::Id;
use crate::index::{IndexedBackend, SharedIndexer};
use crate::repofile::{ConfigFile, SnapshotSummary};

use super::{ParentResult, TreeType};

pub struct TreeArchiver<BE: DecryptWriteBackend, I: IndexedBackend> {
    tree: Tree,
    stack: Vec<(PathBuf, Node, ParentResult<Id>, Tree)>,
    index: I,
    tree_packer: Packer<BE>,
    summary: SnapshotSummary,
}

pub type TreeItem = TreeType<(ParentResult<()>, u64), ParentResult<Id>>;

impl<BE: DecryptWriteBackend, I: IndexedBackend> TreeArchiver<BE, I> {
    pub fn new(
        be: BE,
        index: I,
        indexer: SharedIndexer<BE>,
        config: &ConfigFile,
        summary: SnapshotSummary,
    ) -> Result<Self> {
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

    pub fn add(&mut self, item: TreeItem) -> Result<()> {
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
                    .ok_or_else(|| anyhow!("tree stack empty??"))?;

                // save tree
                trace!("finishing {path:?}");
                let id = self.backup_tree(&path, parent)?;
                node.subtree = Some(id);

                // go back to parent dir
                self.tree = tree;
                self.tree.add(node);
            }
            TreeType::Other((path, node, (parent, size))) => {
                self.add_file(&path, node, parent, size);
            }
        }
        Ok(())
    }

    pub fn add_file(&mut self, path: &Path, node: Node, parent: ParentResult<()>, size: u64) {
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

    pub fn backup_tree(&mut self, path: &Path, parent: ParentResult<Id>) -> Result<Id> {
        let (chunk, id) = self.tree.serialize()?;
        let dirsize = chunk.len() as u64;
        let dirsize_bytes = ByteSize(dirsize).to_string_as(true);

        self.summary.total_dirs_processed += 1;
        self.summary.total_dirsize_processed += dirsize;
        match parent {
            ParentResult::Matched(p_id) if id == p_id => {
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

    pub fn finalize(mut self, parent_tree: Option<Id>) -> Result<(Id, SnapshotSummary)> {
        let parent = match parent_tree {
            None => ParentResult::NotFound,
            Some(id) => ParentResult::Matched(id),
        };
        let id = self.backup_tree(&PathBuf::new(), parent)?;
        let stats = self.tree_packer.finalize()?;
        stats.apply(&mut self.summary, BlobType::Tree);

        Ok((id, self.summary))
    }
}
