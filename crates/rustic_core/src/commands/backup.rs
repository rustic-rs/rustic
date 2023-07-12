//! `backup` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use log::info;

use std::path::PathBuf;

use path_dedot::ParseDot;
use serde::Deserialize;

use crate::{
    archiver::{parent::Parent, Archiver},
    repository::{IndexedIds, IndexedTree},
    DryRunBackend, Id, LocalSource, LocalSourceFilterOptions, LocalSourceSaveOptions, Open,
    PathList, ProgressBars, Repository, RusticResult, SnapshotFile, SnapshotGroup,
    SnapshotGroupCriterion, StdinSource,
};

/// `backup` subcommand
#[cfg_attr(feature = "clap", derive(clap::Parser))]
#[cfg_attr(feature = "merge", derive(merge::Merge))]
#[derive(Clone, Default, Debug, Deserialize)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
// Note: using sources and source within this struct is a hack to support serde(deny_unknown_fields)
// for deserializing the backup options from TOML
// Unfortunately we cannot work with nested flattened structures, see
// https://github.com/serde-rs/serde/issues/1547
// A drawback is that a wrongly set "source(s) = ..." won't get correct error handling and need to be manually checked, see below.
#[allow(clippy::struct_excessive_bools)]
pub struct ParentOpts {
    /// Group snapshots by any combination of host,label,paths,tags to find a suitable parent (default: host,label,paths)
    #[cfg_attr(feature = "clap", clap(long, short = 'g', value_name = "CRITERION",))]
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

impl ParentOpts {
    pub fn get_parent<P: ProgressBars, S: IndexedTree>(
        &self,
        repo: &Repository<P, S>,
        snap: &SnapshotFile,
        backup_stdin: bool,
    ) -> (Option<Id>, Parent) {
        let parent = match (backup_stdin, self.force, &self.parent) {
            (true, _, _) | (false, true, _) => None,
            (false, false, None) => {
                // get suitable snapshot group from snapshot and opts.group_by. This is used to filter snapshots for the parent detection
                let group = SnapshotGroup::from_sn(snap, self.group_by.unwrap_or_default());
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
#[derive(Clone, Default, Debug, Deserialize)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct BackupOpts {
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

    #[cfg_attr(feature = "clap", clap(flatten))]
    #[serde(flatten)]
    pub parent_opts: ParentOpts,

    #[cfg_attr(feature = "clap", clap(flatten))]
    #[serde(flatten)]
    pub ignore_save_opts: LocalSourceSaveOptions,

    #[cfg_attr(feature = "clap", clap(flatten))]
    #[serde(flatten)]
    pub ignore_filter_opts: LocalSourceFilterOptions,
}

pub(crate) fn backup<P: ProgressBars, S: IndexedIds>(
    repo: &Repository<P, S>,
    opts: &BackupOpts,
    source: PathList,
    mut snap: SnapshotFile,
    dry_run: bool,
) -> RusticResult<SnapshotFile> {
    let index = repo.index();

    let backup_stdin = source == PathList::from_string("-", false)?;
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

    let be = DryRunBackend::new(repo.dbe().clone(), dry_run);
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
