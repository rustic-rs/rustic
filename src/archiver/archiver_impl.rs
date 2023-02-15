use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::Local;
use indicatif::ProgressBar;
use log::*;

use crate::backend::{DecryptWriteBackend, ReadSource, ReadSourceEntry};
use crate::blob::{BlobType, Node};
use crate::index::{IndexedBackend, Indexer, SharedIndexer};
use crate::repofile::{ConfigFile, SnapshotFile};

use super::{FileArchiver, Parent, ParentResult, TreeArchiver, TreeIterator};

pub struct Archiver<BE: DecryptWriteBackend, I: IndexedBackend> {
    file_archiver: FileArchiver<BE, I>,
    tree_archiver: TreeArchiver<BE, I>,
    parent: Parent<I>,
    indexer: SharedIndexer<BE>,
    be: BE,
    snap: SnapshotFile,
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

        let file_archiver = FileArchiver::new(be.clone(), index.clone(), indexer.clone(), config)?;
        let tree_archiver = TreeArchiver::new(be.clone(), index, indexer.clone(), config, summary)?;
        Ok(Self {
            file_archiver,
            tree_archiver,
            parent,
            be,
            indexer,
            snap,
        })
    }

    pub fn archive(
        mut self,
        src: impl ReadSource,
        backup_path: &Path,
        as_path: Option<&PathBuf>,
        p: &ProgressBar,
    ) -> Result<SnapshotFile> {
        // filter out errors and handle as_path
        let iter = src.entries().filter_map(|item| match item {
            Err(e) => {
                warn!("ignoring error {}\n", e);
                None
            }
            Ok(ReadSourceEntry { path, node, open }) => {
                let snapshot_path = if let Some(as_path) = as_path {
                    as_path
                        .clone()
                        .join(path.strip_prefix(backup_path).unwrap())
                } else {
                    path
                };
                Some(if node.is_dir() {
                    (snapshot_path, node, open)
                } else {
                    (
                        snapshot_path
                            .parent()
                            .expect("file path should have a parent!")
                            .to_path_buf(),
                        node,
                        open,
                    )
                })
            }
        });
        // handle beginning and ending of trees
        let iter = TreeIterator::new(iter);

        // use parent snapshot
        let iter = iter.filter_map(|item| match self.parent.process(item) {
            Ok(item) => Some(item),
            Err(err) => {
                warn!("ignoring error reading parent snapshot: {err:?}");
                None
            }
        });

        // archive files
        let iter = iter.filter_map(|item| match self.file_archiver.process(item, p.clone()) {
            Ok(item) => Some(item),
            Err(err) => {
                warn!("ignoring error: {err:?}");
                None
            }
        });

        // save items in trees
        for item in iter {
            self.tree_archiver.add(item)?;
        }

        let snap = self.finalize_snapshot()?;
        Ok(snap)
    }

    pub fn backup_reader(
        &mut self,
        path: &Path,
        r: impl Read + Send + 'static,
        node: Node,
        parent: ParentResult<()>,
        p: ProgressBar,
    ) -> Result<()> {
        let (node, filesize) = self.file_archiver.backup_reader(r, node, p)?;
        self.tree_archiver.add_file(path, node, parent, filesize);
        Ok(())
    }

    pub fn finalize_snapshot(mut self) -> Result<SnapshotFile> {
        let stats = self.file_archiver.finalize()?;
        let (id, mut summary) = self.tree_archiver.finalize()?;
        stats.apply(&mut summary, BlobType::Data);
        self.snap.tree = id;

        self.indexer.write().unwrap().finalize()?;

        summary.finalize(self.snap.time)?;
        self.snap.summary = Some(summary);

        let id = self.be.save_file(&self.snap)?;
        self.snap.id = id;

        Ok(self.snap)
    }
}
