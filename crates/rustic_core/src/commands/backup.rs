//! `backup` subcommand
use derive_setters::Setters;
use log::info;

use std::path::PathBuf;

use path_dedot::ParseDot;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

use crate::{
    archiver::{parent::Parent, Archiver},
    backend::ignore::{LocalSource, LocalSourceFilterOptions, LocalSourceSaveOptions},
    backend::{dry_run::DryRunBackend, stdin::StdinSource},
    error::RusticResult,
    id::Id,
    progress::ProgressBars,
    repofile::snapshotfile::{SnapshotGroup, SnapshotGroupCriterion},
    repofile::{PathList, SnapshotFile},
    repository::{IndexedIds, IndexedTree, Repository},
};

/// `backup` subcommand
#[serde_as]
#[cfg_attr(feature = "clap", derive(clap::Parser))]
#[cfg_attr(feature = "merge", derive(merge::Merge))]
#[derive(Clone, Default, Debug, Deserialize, Serialize, Setters)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
#[setters(into)]
#[allow(clippy::struct_excessive_bools)]
#[non_exhaustive]
/// Options how the backup command uses a parent snapshot.
pub struct ParentOptions {
    /// Group snapshots by any combination of host,label,paths,tags to find a suitable parent (default: host,label,paths)
    #[cfg_attr(feature = "clap", clap(long, short = 'g', value_name = "CRITERION",))]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub group_by: Option<SnapshotGroupCriterion>,

    /// Snapshot to use as parent
    #[cfg_attr(
        feature = "clap",
        clap(long, value_name = "SNAPSHOT", conflicts_with = "force",)
    )]
    pub parent: Option<String>,

    /// Use no parent, read all files
    #[cfg_attr(feature = "clap", clap(long, short, conflicts_with = "parent",))]
    #[cfg_attr(feature = "merge", merge(strategy = merge::bool::overwrite_false))]
    pub force: bool,

    /// Ignore ctime changes when checking for modified files
    #[cfg_attr(feature = "clap", clap(long, conflicts_with = "force",))]
    #[cfg_attr(feature = "merge", merge(strategy = merge::bool::overwrite_false))]
    pub ignore_ctime: bool,

    /// Ignore inode number changes when checking for modified files
    #[cfg_attr(feature = "clap", clap(long, conflicts_with = "force",))]
    #[cfg_attr(feature = "merge", merge(strategy = merge::bool::overwrite_false))]
    pub ignore_inode: bool,
}

impl ParentOptions {
    /// Get parent snapshot.
    ///
    /// # Type Parameters
    ///
    /// * `P` - The type of the progress bars.
    /// * `S` - The type of the indexed tree.
    ///
    /// # Arguments
    ///
    /// * `repo` - The repository to use
    /// * `snap` - The snapshot to use
    /// * `backup_stdin` - Whether the backup is from stdin
    ///
    /// # Returns
    ///
    /// The parent snapshot id and the parent object or `None` if no parent is used.
    pub(crate) fn get_parent<P: ProgressBars, S: IndexedTree>(
        &self,
        repo: &Repository<P, S>,
        snap: &SnapshotFile,
        backup_stdin: bool,
    ) -> (Option<Id>, Parent) {
        let parent = match (backup_stdin, self.force, &self.parent) {
            (true, _, _) | (false, true, _) => None,
            (false, false, None) => {
                // get suitable snapshot group from snapshot and opts.group_by. This is used to filter snapshots for the parent detection
                let group = SnapshotGroup::from_snapshot(snap, self.group_by.unwrap_or_default());
                SnapshotFile::latest(
                    repo.dbe(),
                    |snap| snap.has_group(&group),
                    &repo.pb.progress_counter(""),
                )
                .ok()
            }
            (false, false, Some(parent)) => SnapshotFile::from_id(repo.dbe(), parent).ok(),
        };

        let (parent_tree, parent_id) = parent.map(|parent| (parent.tree, parent.id)).unzip();

        (
            parent_id,
            Parent::new(
                repo.index(),
                parent_tree,
                self.ignore_ctime,
                self.ignore_inode,
            ),
        )
    }
}

#[cfg_attr(feature = "clap", derive(clap::Parser))]
#[cfg_attr(feature = "merge", derive(merge::Merge))]
#[derive(Clone, Default, Debug, Deserialize, Serialize, Setters)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
#[setters(into)]
#[non_exhaustive]
/// Options for the `backup` command.
pub struct BackupOptions {
    /// Set filename to be used when backing up from stdin
    #[cfg_attr(
        feature = "clap",
        clap(long, value_name = "FILENAME", default_value = "stdin")
    )]
    #[cfg_attr(feature = "merge", merge(skip))]
    pub stdin_filename: String,

    /// Manually set backup path in snapshot
    #[cfg_attr(feature = "clap", clap(long, value_name = "PATH"))]
    pub as_path: Option<PathBuf>,

    /// Dry-run mode: Don't write any data or snapshot
    #[cfg_attr(feature = "clap", clap(long))]
    #[cfg_attr(feature = "merge", merge(strategy = merge::bool::overwrite_false))]
    pub dry_run: bool,

    #[cfg_attr(feature = "clap", clap(flatten))]
    #[serde(flatten)]
    /// Options how to use a parent snapshot
    pub parent_opts: ParentOptions,

    #[cfg_attr(feature = "clap", clap(flatten))]
    #[serde(flatten)]
    /// Options how to save entries from a local source
    pub ignore_save_opts: LocalSourceSaveOptions,

    #[cfg_attr(feature = "clap", clap(flatten))]
    #[serde(flatten)]
    /// Options how to filter from a local source
    pub ignore_filter_opts: LocalSourceFilterOptions,
}

/// Backup data, create a snapshot.
///
/// # Type Parameters
///
/// * `P` - The type of the progress bars.
/// * `S` - The type of the indexed tree.
///
/// # Arguments
///
/// * `repo` - The repository to use
/// * `opts` - The backup options
/// * `source` - The source to backup
/// * `snap` - The snapshot to backup
///
/// # Errors
///
/// * [`PackerErrorKind::SendingCrossbeamMessageFailed`] - If sending the message to the raw packer fails.
/// * [`PackerErrorKind::IntConversionFailed`] - If converting the data length to u64 fails
/// * [`PackerErrorKind::SendingCrossbeamMessageFailed`] - If sending the message to the raw packer fails.
/// * [`CryptBackendErrorKind::SerializingToJsonByteVectorFailed`] - If the index file could not be serialized.
/// * [`SnapshotFileErrorKind::OutOfRange`] - If the time is not in the range of `Local::now()`
///
/// # Returns
///
/// The snapshot pointing to the backup'ed data.
///
/// [`PackerErrorKind::SendingCrossbeamMessageFailed`]: crate::error::PackerErrorKind::SendingCrossbeamMessageFailed
/// [`PackerErrorKind::IntConversionFailed`]: crate::error::PackerErrorKind::IntConversionFailed
/// [`PackerErrorKind::SendingCrossbeamMessageFailed`]: crate::error::PackerErrorKind::SendingCrossbeamMessageFailed
/// [`CryptBackendErrorKind::SerializingToJsonByteVectorFailed`]: crate::error::CryptBackendErrorKind::SerializingToJsonByteVectorFailed
/// [`SnapshotFileErrorKind::OutOfRange`]: crate::error::SnapshotFileErrorKind::OutOfRange
pub(crate) fn backup<P: ProgressBars, S: IndexedIds>(
    repo: &Repository<P, S>,
    opts: &BackupOptions,
    source: PathList,
    mut snap: SnapshotFile,
) -> RusticResult<SnapshotFile> {
    let index = repo.index();

    let backup_stdin = source == PathList::from_string("-")?;
    let backup_path = if backup_stdin {
        vec![PathBuf::from(&opts.stdin_filename)]
    } else {
        source.paths()
    };

    let as_path = opts
        .as_path
        .as_ref()
        .map(|p| -> RusticResult<_> { Ok(p.parse_dot()?.to_path_buf()) })
        .transpose()?;

    match &as_path {
        Some(p) => snap.paths.set_paths(&[p.clone()])?,
        None => snap.paths.set_paths(&backup_path)?,
    };

    let (parent_id, parent) = opts.parent_opts.get_parent(repo, &snap, backup_stdin);
    match parent_id {
        Some(id) => {
            info!("using parent {}", id);
            snap.parent = Some(id);
        }
        None => {
            info!("using no parent");
        }
    };

    let be = DryRunBackend::new(repo.dbe().clone(), opts.dry_run);
    info!("starting to backup {source}...");
    let archiver = Archiver::new(be, index.clone(), repo.config(), parent, snap)?;
    let p = repo.pb.progress_bytes("determining size...");

    let snap = if backup_stdin {
        let path = &backup_path[0];
        let src = StdinSource::new(path.clone())?;
        archiver.archive(repo.index(), src, path, as_path.as_ref(), &p)?
    } else {
        let src = LocalSource::new(
            opts.ignore_save_opts,
            &opts.ignore_filter_opts,
            &backup_path,
        )?;
        archiver.archive(repo.index(), src, &backup_path[0], as_path.as_ref(), &p)?
    };

    Ok(snap)
}
