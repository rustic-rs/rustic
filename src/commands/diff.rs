//! `diff` subcommand

use crate::{Application, RUSTIC_APP, repository::CliIndexedRepo, status_err};

use abscissa_core::{Command, Runnable, Shutdown};
use clap::ValueHint;
use log::debug;

use std::{fmt::Display, path::PathBuf};

use anyhow::{Context, Result, bail};

use rustic_core::{
    IndexedFull, LocalDestination, LocalSource, LocalSourceFilterOptions, LocalSourceSaveOptions,
    LsOptions, ReadSource, ReadSourceEntry, Repository, RusticResult,
    repofile::{Node, NodeType},
    typed_path::{UnixPath, UnixPathBuf},
    util::path_to_unix_path,
};

/// `diff` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct DiffCmd {
    /// Reference snapshot/path
    #[clap(value_name = "SNAPSHOT1[:PATH1]")]
    snap1: String,

    /// New snapshot/path (uses PATH2 = PATH1, if not given; uses local path if no snapshot is given)
    #[clap(value_name = "SNAPSHOT2[:PATH2]|PATH2", value_hint = ValueHint::AnyPath)]
    snap2: Option<String>,

    /// show differences in metadata
    #[clap(long)]
    metadata: bool,

    /// don't check for different file contents
    #[clap(long)]
    no_content: bool,

    /// only show differences for identical files, this can be used for a bitrot test on the local path
    #[clap(long, conflicts_with = "no_content")]
    only_identical: bool,

    /// Ignore options
    #[clap(flatten)]
    ignore_opts: LocalSourceFilterOptions,
}

impl Runnable for DiffCmd {
    fn run(&self) {
        if let Err(err) = RUSTIC_APP
            .config()
            .repository
            .run_indexed(|repo| self.inner_run(repo))
        {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl DiffCmd {
    fn inner_run(&self, repo: CliIndexedRepo) -> Result<()> {
        let config = RUSTIC_APP.config();

        let (id1, path1) = arg_to_snap_path(&self.snap1, "");
        let (id2, path2) = self
            .snap2
            .as_ref()
            .map_or((None, path1), |snap2| arg_to_snap_path(snap2, path1));

        match (id1, id2) {
            (Some(id1), Some(id2)) => {
                // diff between two snapshots
                let snaps = repo.get_snapshots(&[id1, id2])?;

                let snap1 = &snaps[0];
                let snap2 = &snaps[1];

                let node1 = repo.node_from_snapshot_and_path(snap1, path1)?;
                let node2 = repo.node_from_snapshot_and_path(snap2, path2)?;

                diff(
                    repo.ls(&node1, &LsOptions::default())?,
                    repo.ls(&node2, &LsOptions::default())?,
                    self.no_content,
                    |_path, node1, node2| Ok(node1.content == node2.content),
                    self.metadata,
                )?;
            }
            (Some(id1), None) => {
                // diff between snapshot and local path
                let snap1 =
                    repo.get_snapshot_from_str(id1, |sn| config.snapshot_filter.matches(sn))?;

                let node1 = repo.node_from_snapshot_and_path(&snap1, path1)?;
                let local = LocalDestination::new(path2, false, !node1.is_dir())?;
                let path2 = PathBuf::from(path2);
                let path2_unix = path_to_unix_path(&path2)?;
                let is_dir = path2
                    .metadata()
                    .with_context(|| format!("Error accessing {path2:?}"))?
                    .is_dir();
                let src = LocalSource::new(
                    LocalSourceSaveOptions::default(),
                    &self.ignore_opts,
                    &[&path2],
                )?
                .entries()
                .map(|item| -> RusticResult<_> {
                    let ReadSourceEntry { path, node, .. } = item?;
                    let path = if is_dir {
                        // remove given path prefix for dirs as local path
                        path.strip_prefix(&path2_unix).unwrap().to_path_buf()
                    } else {
                        // ensure that we really get the filename if local path is a file
                        path2_unix.file_name().unwrap().into()
                    };
                    Ok((path, node))
                });

                if self.only_identical {
                    diff_identical(
                        repo.ls(&node1, &LsOptions::default())?,
                        src,
                        |path, node1, _node2| identical_content_local(&local, &repo, path, node1),
                    )?;
                } else {
                    diff(
                        repo.ls(&node1, &LsOptions::default())?,
                        src,
                        self.no_content,
                        |path, node1, _node2| identical_content_local(&local, &repo, path, node1),
                        self.metadata,
                    )?;
                }
            }
            (None, _) => {
                bail!("cannot use local path as first argument");
            }
        };

        Ok(())
    }
}

/// Split argument into snapshot id and path
///
/// # Arguments
///
/// * `arg` - argument to split
/// * `default_path` - default path if no path is given
///
/// # Returns
///
/// A tuple of the snapshot id and the path
fn arg_to_snap_path<'a>(arg: &'a str, default_path: &'a str) -> (Option<&'a str>, &'a str) {
    match arg.split_once(':') {
        Some(("local", path)) => (None, path),
        Some((id, path)) => (Some(id), path),
        None => {
            if arg.contains('/') {
                (None, arg)
            } else {
                (Some(arg), default_path)
            }
        }
    }
}

/// Check if the content of a file in a snapshot is identical to the content of a local file
///
/// # Arguments
///
/// * `local` - local destination
/// * `repo` - repository
/// * `path` - path of the file in the snapshot
/// * `node` - node of the file in the snapshot
///
/// # Errors
///
/// * [`RepositoryErrorKind::IdNotFound`] - If the id of a blob is not found in the repository
///
/// # Returns
///
/// `true` if the content of the file in the snapshot is identical to the content of the local file,
/// `false` otherwise
///
/// [`RepositoryErrorKind::IdNotFound`]: rustic_core::error::RepositoryErrorKind::IdNotFound
fn identical_content_local<P, S: IndexedFull>(
    local: &LocalDestination,
    repo: &Repository<P, S>,
    path: &UnixPath,
    node: &Node,
) -> Result<bool> {
    let Some(mut open_file) = local.get_matching_file(path, node.meta.size) else {
        return Ok(false);
    };

    for id in node.content.iter().flatten() {
        let ie = repo.get_index_entry(id)?;
        let length = ie.data_length();
        if !id.blob_matches_reader(length as usize, &mut open_file) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Statistics about the differences listed with the [`DiffCmd`] command
#[derive(Default)]
struct DiffStatistics {
    files_added: usize,
    files_removed: usize,
    files_changed: usize,
    directories_added: usize,
    directories_removed: usize,
    others_added: usize,
    others_removed: usize,
    node_type_changed: usize,
    metadata_changed: usize,
    symlink_added: usize,
    symlink_removed: usize,
    symlink_changed: usize,
}

impl DiffStatistics {
    fn removed_node(&mut self, node_type: &NodeType) {
        match node_type {
            NodeType::File => self.files_removed += 1,
            NodeType::Dir => self.directories_removed += 1,
            NodeType::Symlink { .. } => self.symlink_removed += 1,
            _ => self.others_removed += 1,
        }
    }

    fn added_node(&mut self, node_type: &NodeType) {
        match node_type {
            NodeType::File => self.files_added += 1,
            NodeType::Dir => self.directories_added += 1,
            NodeType::Symlink { .. } => self.symlink_added += 1,
            _ => self.others_added += 1,
        }
    }

    fn changed_file(&mut self) {
        self.files_changed += 1;
    }

    fn changed_node_type(&mut self) {
        self.node_type_changed += 1;
    }

    fn changed_metadata(&mut self) {
        self.metadata_changed += 1;
    }

    fn changed_symlink(&mut self) {
        self.symlink_changed += 1;
    }
}

impl Display for DiffStatistics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "Files   :\t{} new,\t{} removed,\t{} changed\n",
            self.files_added, self.files_removed, self.files_changed
        ))?;
        // symlink
        if self.symlink_added != 0 || self.symlink_removed != 0 || self.symlink_changed != 0 {
            f.write_fmt(format_args!(
                "Symlinks:\t{} new,\t{} removed,\t{} changed\n",
                self.symlink_added, self.symlink_removed, self.symlink_changed
            ))?;
        }
        f.write_fmt(format_args!(
            "Dirs    :\t{} new,\t{} removed\n",
            self.directories_added, self.directories_removed
        ))?;
        if self.others_added != 0 || self.others_removed != 0 {
            f.write_fmt(format_args!(
                "Others  :\t{} new,\t{} removed\n",
                self.others_added, self.others_removed
            ))?;
        }

        // node type
        if self.node_type_changed != 0 {
            f.write_fmt(format_args!(
                "NodeType:\t{} changed\n",
                self.node_type_changed
            ))?;
        }

        // metadata
        if self.metadata_changed != 0 {
            f.write_fmt(format_args!(
                "Metadata:\t{} changed\n",
                self.metadata_changed
            ))?;
        }
        Ok(())
    }
}

/// Compare two streams of nodes and print the differences
///
/// # Arguments
///
/// * `tree_streamer1` - first stream of nodes
/// * `tree_streamer2` - second stream of nodes
/// * `no_content` - don't check for different file contents
/// * `file_identical` - function to check if the content of two files is identical
/// * `metadata` - show differences in metadata
///
/// # Errors
///
// TODO!: add errors!
fn diff(
    mut tree_streamer1: impl Iterator<Item = RusticResult<(UnixPathBuf, Node)>>,
    mut tree_streamer2: impl Iterator<Item = RusticResult<(UnixPathBuf, Node)>>,
    no_content: bool,
    file_identical: impl Fn(&UnixPath, &Node, &Node) -> Result<bool>,
    metadata: bool,
) -> Result<()> {
    let mut item1 = tree_streamer1.next().transpose()?;
    let mut item2 = tree_streamer2.next().transpose()?;

    let mut diff_statistics = DiffStatistics::default();

    loop {
        match (&item1, &item2) {
            (None, None) => break,
            (Some(i1), None) => {
                println!("-    {:?}", i1.0);
                diff_statistics.removed_node(&i1.1.node_type);
                item1 = tree_streamer1.next().transpose()?;
            }
            (None, Some(i2)) => {
                println!("+    {:?}", i2.0);
                diff_statistics.added_node(&i2.1.node_type);
                item2 = tree_streamer2.next().transpose()?;
            }
            (Some(i1), Some(i2)) if i1.0 < i2.0 => {
                println!("-    {:?}", i1.0);
                diff_statistics.removed_node(&i1.1.node_type);
                item1 = tree_streamer1.next().transpose()?;
            }
            (Some(i1), Some(i2)) if i1.0 > i2.0 => {
                println!("+    {:?}", i2.0);
                diff_statistics.added_node(&i2.1.node_type);
                item2 = tree_streamer2.next().transpose()?;
            }
            (Some(i1), Some(i2)) => {
                let path = &i1.0;
                let node1 = &i1.1;
                let node2 = &i2.1;

                let are_both_symlink = matches!(&node1.node_type, NodeType::Symlink { .. })
                    && matches!(&node2.node_type, NodeType::Symlink { .. });
                match &node1.node_type {
                    // if node1.node_type != node2.node_type, they could be different symlinks,
                    // for this reason we check:
                    // that their type is different AND that they are not both symlinks
                    tpe if tpe != &node2.node_type && !are_both_symlink => {
                        // type was changed
                        println!("T    {path:?}");
                        diff_statistics.changed_node_type();
                    }
                    NodeType::File if !no_content && !file_identical(path, node1, node2)? => {
                        println!("M    {path:?}");
                        diff_statistics.changed_file();
                    }
                    NodeType::File if metadata && node1.meta != node2.meta => {
                        println!("U    {path:?}");
                        diff_statistics.changed_metadata();
                    }
                    NodeType::Symlink { .. } => {
                        if node1.node_type.to_link() != node2.node_type.to_link() {
                            println!("U    {path:?}");
                            diff_statistics.changed_symlink();
                        }
                    }
                    _ => {} // no difference to show
                }
                item1 = tree_streamer1.next().transpose()?;
                item2 = tree_streamer2.next().transpose()?;
            }
        }
    }
    println!("{diff_statistics}");
    Ok(())
}

fn diff_identical(
    mut tree_streamer1: impl Iterator<Item = RusticResult<(UnixPathBuf, Node)>>,
    mut tree_streamer2: impl Iterator<Item = RusticResult<(UnixPathBuf, Node)>>,
    file_identical: impl Fn(&UnixPath, &Node, &Node) -> Result<bool>,
) -> Result<()> {
    let mut item1 = tree_streamer1.next().transpose()?;
    let mut item2 = tree_streamer2.next().transpose()?;

    let mut checked: usize = 0;

    loop {
        match (&item1, &item2) {
            (None, None) => break,
            (Some(i1), None) => {
                let path = &i1.0;
                debug!("not checking {}: not present in target", path.display());
                item1 = tree_streamer1.next().transpose()?;
            }
            (None, Some(i2)) => {
                let path = &i2.0;
                debug!("not checking {}: not present in source", path.display());
                item2 = tree_streamer2.next().transpose()?;
            }
            (Some(i1), Some(i2)) if i1.0 < i2.0 => {
                let path = &i1.0;
                debug!("not checking {}: not present in target", path.display());
                item1 = tree_streamer1.next().transpose()?;
            }
            (Some(i1), Some(i2)) if i1.0 > i2.0 => {
                let path = &i2.0;
                debug!("not checking {}: not present in source", path.display());
                item2 = tree_streamer2.next().transpose()?;
            }
            (Some(i1), Some(i2)) => {
                let path = &i1.0;
                let node1 = &i1.1;
                let node2 = &i2.1;

                if matches!(&node1.node_type, NodeType::File)
                    && matches!(&node2.node_type, NodeType::File)
                    && node1.meta == node2.meta
                {
                    debug!("checking {}", path.display());
                    checked += 1;
                    if !file_identical(path, node1, node2)? {
                        println!("M    {path:?}");
                    }
                } else {
                    debug!("not checking {}: metadata changed", path.display());
                }
                item1 = tree_streamer1.next().transpose()?;
                item2 = tree_streamer2.next().transpose()?;
            }
        }
    }
    println!("checked {checked} files.");
    Ok(())
}
