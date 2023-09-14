pub(crate) mod file_archiver;
pub(crate) mod parent;
pub(crate) mod tree;
pub(crate) mod tree_archiver;

use std::path::{Path, PathBuf};

use chrono::Local;
use log::warn;
use pariter::{scope, IteratorExt};

use crate::{
    archiver::{
        file_archiver::FileArchiver, parent::Parent, tree::TreeIterator,
        tree_archiver::TreeArchiver,
    },
    backend::{decrypt::DecryptWriteBackend, ReadSource, ReadSourceEntry},
    blob::BlobType,
    index::{indexer::Indexer, indexer::SharedIndexer, IndexedBackend},
    repofile::{configfile::ConfigFile, snapshotfile::SnapshotFile},
    Progress, RusticResult,
};

/// The `Archiver` is responsible for archiving files and trees.
/// It will read the file, chunk it, and write the chunks to the backend.
///
/// # Type Parameters
///
/// * `BE` - The backend type.
/// * `I` - The index to read from.
#[allow(missing_debug_implementations)]
pub struct Archiver<BE: DecryptWriteBackend, I: IndexedBackend> {
    /// The `FileArchiver` is responsible for archiving files.
    file_archiver: FileArchiver<BE, I>,

    /// The `TreeArchiver` is responsible for archiving trees.
    tree_archiver: TreeArchiver<BE, I>,

    /// The parent snapshot to use.
    parent: Parent,

    /// The SharedIndexer is used to index the data.
    indexer: SharedIndexer<BE>,

    /// The backend to write to.
    be: BE,

    /// The SnapshotFile to write to.
    snap: SnapshotFile,
}

impl<BE: DecryptWriteBackend, I: IndexedBackend> Archiver<BE, I> {
    /// Creates a new `Archiver`.
    ///
    /// # Arguments
    ///
    /// * `be` - The backend to write to.
    /// * `index` - The index to read from.
    /// * `config` - The config file.
    /// * `parent` - The parent snapshot to use.
    /// * `snap` - The `SnapshotFile` to write to.
    ///
    /// # Errors
    ///
    /// * [`PackerErrorKind::SendingCrossbeamMessageFailed`] - If sending the message to the raw packer fails.
    /// * [`PackerErrorKind::IntConversionFailed`] - If converting the data length to u64 fails
    ///
    /// [`PackerErrorKind::SendingCrossbeamMessageFailed`]: crate::error::PackerErrorKind::SendingCrossbeamMessageFailed
    /// [`PackerErrorKind::IntConversionFailed`]: crate::error::PackerErrorKind::IntConversionFailed
    pub fn new(
        be: BE,
        index: I,
        config: &ConfigFile,
        parent: Parent,
        mut snap: SnapshotFile,
    ) -> RusticResult<Self> {
        let indexer = Indexer::new(be.clone()).into_shared();
        let mut summary = snap.summary.take().unwrap_or_default();
        summary.backup_start = Local::now();

        let file_archiver = FileArchiver::new(be.clone(), index.clone(), indexer.clone(), config)?;
        let tree_archiver = TreeArchiver::new(be.clone(), index, indexer.clone(), config, summary)?;
        Ok(Self {
            file_archiver,
            tree_archiver,
            parent,
            indexer,
            be,
            snap,
        })
    }

    /// Archives the given source.
    ///
    /// This will archive all files and trees in the given source.
    ///
    /// # Type Parameters
    ///
    /// * `R` - The type of the source.
    ///
    /// # Arguments
    ///
    /// * `index` - The index to read from.
    /// * `src` - The source to archive.
    /// * `backup_path` - The path to the backup.
    /// * `as_path` - The path to archive the backup as.
    /// * `p` - The progress bar.
    ///
    /// # Errors
    ///
    /// * [`PackerErrorKind::SendingCrossbeamMessageFailed`] - If sending the message to the raw packer fails.
    /// * [`CryptBackendErrorKind::SerializingToJsonByteVectorFailed`] - If the index file could not be serialized.
    /// * [`SnapshotFileErrorKind::OutOfRange`] - If the time is not in the range of `Local::now()`
    ///
    /// [`PackerErrorKind::SendingCrossbeamMessageFailed`]: crate::error::PackerErrorKind::SendingCrossbeamMessageFailed
    /// [`CryptBackendErrorKind::SerializingToJsonByteVectorFailed`]: crate::error::CryptBackendErrorKind::SerializingToJsonByteVectorFailed
    /// [`SnapshotFileErrorKind::OutOfRange`]: crate::error::SnapshotFileErrorKind::OutOfRange
    pub fn archive<R>(
        mut self,
        index: &I,
        src: R,
        backup_path: &Path,
        as_path: Option<&PathBuf>,
        p: &impl Progress,
    ) -> RusticResult<SnapshotFile>
    where
        R: ReadSource + 'static,
        <R as ReadSource>::Open: Send,
        <R as ReadSource>::Iter: Send,
    {
        if !p.is_hidden() {
            if let Some(size) = src.size()? {
                p.set_length(size);
            }
        };
        p.set_title("backing up...");

        // filter out errors and handle as_path
        let iter = src.entries().filter_map(|item| match item {
            Err(e) => {
                warn!("ignoring error {e}\n");
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

        scope(|scope| -> RusticResult<_> {
            // use parent snapshot
            iter.filter_map(|item| match self.parent.process(index, item) {
                Ok(item) => Some(item),
                Err(err) => {
                    warn!("ignoring error reading parent snapshot: {err:?}");
                    None
                }
            })
            // archive files in parallel
            .parallel_map_scoped(scope, |item| self.file_archiver.process(item, p))
            .readahead_scoped(scope)
            .filter_map(|item| match item {
                Ok(item) => Some(item),
                Err(err) => {
                    warn!("ignoring error: {err:?}");
                    None
                }
            })
            .try_for_each(|item| self.tree_archiver.add(item))
        })
        .unwrap()?;

        let stats = self.file_archiver.finalize()?;
        let (id, mut summary) = self.tree_archiver.finalize(self.parent.tree_id())?;
        stats.apply(&mut summary, BlobType::Data);
        self.snap.tree = id;

        self.indexer.write().unwrap().finalize()?;

        summary.finalize(self.snap.time)?;
        self.snap.summary = Some(summary);

        let id = self.be.save_file(&self.snap)?;
        self.snap.id = id;

        p.finish();
        Ok(self.snap)
    }
}
