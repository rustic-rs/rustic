use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use bytesize::ByteSize;
use chrono::Local;
use futures::{stream::FuturesOrdered, StreamExt};
use indicatif::ProgressBar;
use log::*;
use tokio::spawn;

use crate::backend::DecryptWriteBackend;
use crate::blob::{BlobType, Metadata, Node, NodeType, Packer, Tree};
use crate::chunker::ChunkIter;
use crate::crypto::hash;
use crate::id::Id;
use crate::index::{IndexedBackend, Indexer, SharedIndexer};
use crate::repo::{ConfigFile, SnapshotFile, SnapshotSummary};

use super::{Parent, ParentResult};

pub struct Archiver<BE: DecryptWriteBackend, I: IndexedBackend> {
    path: PathBuf,
    tree: Tree,
    parent: Parent<I>,
    stack: Vec<(Node, Tree, Parent<I>)>,
    index: I,
    indexer: SharedIndexer<BE>,
    data_packer: Packer<BE>,
    tree_packer: Packer<BE>,
    be: BE,
    poly: u64,
    snap: SnapshotFile,
    summary: SnapshotSummary,
}

impl<BE: DecryptWriteBackend, I: IndexedBackend> Archiver<BE, I> {
    pub fn new(
        be: BE,
        index: I,
        config: &ConfigFile,
        parent: Parent<I>,
        mut snap: SnapshotFile,
    ) -> Result<Self> {
        let indexer = Indexer::new(be.clone()).into_shared();
        let mut summary = snap.summary.take().unwrap();
        summary.backup_start = Local::now();
        let poly = config.poly()?;

        let data_packer = Packer::new(
            be.clone(),
            BlobType::Data,
            indexer.clone(),
            config,
            index.total_size(&BlobType::Data),
        )?;
        let tree_packer = Packer::new(
            be.clone(),
            BlobType::Tree,
            indexer.clone(),
            config,
            index.total_size(&BlobType::Tree),
        )?;
        Ok(Self {
            path: PathBuf::from("/"),
            tree: Tree::new(),
            parent,
            stack: Vec::new(),
            index,
            data_packer,
            tree_packer,
            be,
            poly,
            indexer,
            snap,
            summary,
        })
    }

    pub fn add_file(&mut self, node: Node, size: u64) {
        let filename = self.path.join(node.name());
        match self.parent.is_parent(&node) {
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
        self.tree.add(node);
        self.summary.total_files_processed += 1;
        self.summary.total_bytes_processed += size;
    }

    pub fn add_dir(&mut self, node: Node, size: u64) {
        self.tree.add(node);
        self.summary.total_dirs_processed += 1;
        self.summary.total_dirsize_processed += size;
    }

    pub async fn add_entry(&mut self, path: &Path, node: Node, p: ProgressBar) -> Result<()> {
        let basepath = if node.is_dir() {
            path
        } else {
            path.parent()
                .ok_or_else(|| anyhow!("file path should have a parent!"))?
        };

        self.finish_trees(basepath).await?;

        let missing_dirs = basepath.strip_prefix(&self.path)?;
        for p in missing_dirs.iter() {
            // new subdir
            self.path.push(p);
            let tree = std::mem::replace(&mut self.tree, Tree::new());
            if self.path == path {
                // use Node and return
                let new_parent = self.parent.sub_parent(&node)?;
                let parent = std::mem::replace(&mut self.parent, new_parent);
                self.stack.push((node, tree, parent));
                return Ok(());
            } else {
                let node = Node::new_node(p, NodeType::Dir, Metadata::default());
                let new_parent = self.parent.sub_parent(&node)?;
                let parent = std::mem::replace(&mut self.parent, new_parent);
                self.stack.push((node, tree, parent));
            };
        }

        match node.node_type() {
            NodeType::File => {
                self.backup_file(path, node, p).await?;
            }
            NodeType::Dir => {}          // is already handled, see above
            _ => self.add_file(node, 0), // all other cases: just save the given node
        }
        Ok(())
    }

    pub async fn finish_trees(&mut self, path: &Path) -> Result<()> {
        while !path.starts_with(&self.path) {
            // save tree and go back to parent dir
            let (chunk, id) = self.tree.serialize()?;

            let (mut node, tree, parent) = self
                .stack
                .pop()
                .ok_or_else(|| anyhow!("tree stack empty??"))?;

            node.set_subtree(id);
            self.tree = tree;
            self.parent = parent;

            self.backup_tree(node, chunk).await?;
            self.path.pop();
        }
        Ok(())
    }

    pub async fn backup_tree(&mut self, node: Node, chunk: Vec<u8>) -> Result<()> {
        let dirsize = chunk.len() as u64;
        let dirsize_bytes = ByteSize(dirsize).to_string_as(true);
        let id = node.subtree().unwrap();

        match self.parent.is_parent(&node) {
            ParentResult::Matched(p_node) if node.subtree() == p_node.subtree() => {
                debug!("unchanged tree: {:?}", self.path);
                self.add_dir(node, dirsize);
                self.summary.dirs_unmodified += 1;
                return Ok(());
            }
            ParentResult::NotFound => {
                debug!("new       tree: {:?} {}", self.path, dirsize_bytes);
                self.summary.dirs_new += 1;
            }
            _ => {
                // "Matched" trees where the subree id does not match or unmach
                debug!("changed   tree: {:?} {}", self.path, dirsize_bytes);
                self.summary.dirs_changed += 1;
            }
        }

        if !self.index.has_tree(&id) {
            match self.tree_packer.add(&chunk, &id).await? {
                0 => {}
                packed_size => {
                    self.summary.tree_blobs += 1;
                    self.summary.data_added += dirsize;
                    self.summary.data_added_packed += packed_size;
                    self.summary.data_added_trees += dirsize;
                    self.summary.data_added_trees_packed += packed_size;
                }
            }
        }
        self.add_dir(node, dirsize);
        Ok(())
    }

    pub async fn backup_file(&mut self, path: &Path, node: Node, p: ProgressBar) -> Result<()> {
        if let ParentResult::Matched(p_node) = self.parent.is_parent(&node) {
            if p_node.content().iter().all(|id| self.index.has_data(id)) {
                let size = *p_node.meta().size();
                let mut node = node;
                node.set_content(p_node.content().to_vec());
                self.add_file(node, size);
                p.inc(size);
                return Ok(());
            } else {
                warn!(
                    "missing blobs in index for unchanged file {:?}; re-reading file",
                    self.path.join(node.name())
                );
            }
        }
        let f = File::open(path)?;
        self.backup_reader(f, node, p).await
    }

    pub async fn backup_reader(&mut self, r: impl Read, node: Node, p: ProgressBar) -> Result<()> {
        let chunk_iter = ChunkIter::new(r, *node.meta().size() as usize, &self.poly);
        let mut content = Vec::new();
        let mut filesize: u64 = 0;

        let mut queue = FuturesOrdered::new();

        for chunk in chunk_iter {
            let chunk = chunk?;
            let size = chunk.len() as u64;
            filesize += size;

            queue.push_back(spawn(async move {
                let id = hash(&chunk);
                (id, chunk, size)
            }));

            if queue.len() > 8 {
                let (id, chunk, size) = queue.next().await.unwrap()?;
                self.process_data_junk(id, &chunk, size, &p).await?;
                content.push(id);
            }
        }

        while let Some(Ok((id, chunk, size))) = queue.next().await {
            self.process_data_junk(id, &chunk, size, &p).await?;
            content.push(id);
        }

        let mut node = node;
        node.set_content(content);
        self.add_file(node, filesize);
        Ok(())
    }

    async fn process_data_junk(
        &mut self,
        id: Id,
        chunk: &[u8],
        size: u64,
        p: &ProgressBar,
    ) -> Result<()> {
        if !self.index.has_data(&id) {
            match self.data_packer.add(chunk, &id).await? {
                0 => {}
                packed_size => {
                    self.summary.data_blobs += 1;
                    self.summary.data_added += size;
                    self.summary.data_added_packed += packed_size;
                    self.summary.data_added_files += size;
                    self.summary.data_added_files_packed += packed_size;
                }
            }
        }
        p.inc(size);
        Ok(())
    }

    pub async fn finalize_snapshot(mut self) -> Result<SnapshotFile> {
        self.finish_trees(&PathBuf::from("/")).await?;

        let (chunk, id) = self.tree.serialize()?;
        if !self.index.has_tree(&id) {
            self.tree_packer.add(&chunk, &id).await?;
        }
        self.snap.tree = id;

        self.data_packer.finalize().await?;
        self.tree_packer.finalize().await?;
        {
            let indexer = self.indexer.write().await;
            indexer.finalize()?;
        }
        let end_time = Local::now();
        self.summary.backup_duration = (end_time - self.summary.backup_start)
            .to_std()?
            .as_secs_f64();
        self.summary.total_duration = (end_time - self.snap.time).to_std()?.as_secs_f64();
        self.summary.backup_end = end_time;
        self.snap.summary = Some(self.summary);
        let id = self.be.save_file(&self.snap)?;
        self.snap.id = id;

        Ok(self.snap)
    }
}
