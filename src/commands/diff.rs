//! `diff` subcommand

use crate::{commands::open_repository_indexed, status_err, Application, RUSTIC_APP};

use abscissa_core::{Command, Runnable, Shutdown};

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use rustic_core::{
    repofile::{BlobType, Node, NodeType},
    IndexedFull, LocalDestination, LocalSource, LocalSourceFilterOptions, LocalSourceSaveOptions,
    LsOptions, ReadSource, ReadSourceEntry, Repository, RusticResult,
};

/// `diff` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct DiffCmd {
    /// Reference snapshot/path
    #[clap(value_name = "SNAPSHOT1[:PATH1]")]
    snap1: String,

    /// New snapshot/path or local path [default for PATH2: PATH1]
    #[clap(value_name = "SNAPSHOT2[:PATH2]|PATH2")]
    snap2: String,

    /// show differences in metadata
    #[clap(long)]
    metadata: bool,

    /// don't check for different file contents
    #[clap(long)]
    no_content: bool,

    /// Ignore options
    #[clap(flatten)]
    ignore_opts: LocalSourceFilterOptions,
}

impl Runnable for DiffCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl DiffCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();
        let repo = open_repository_indexed(&config.repository)?;

        let (id1, path1) = arg_to_snap_path(&self.snap1, "");
        let (id2, path2) = arg_to_snap_path(&self.snap2, path1);

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
                        path.strip_prefix(&path2).unwrap().to_path_buf()
                    } else {
                        // ensure that we really get the filename if local path is a file
                        path2.file_name().unwrap().into()
                    };
                    Ok((path, node))
                });

                diff(
                    repo.ls(&node1, &LsOptions::default())?,
                    src,
                    self.no_content,
                    |path, node1, _node2| identical_content_local(&local, &repo, path, node1),
                    self.metadata,
                )?;
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
    path: &Path,
    node: &Node,
) -> Result<bool> {
    let Some(mut open_file) = local.get_matching_file(path, node.meta.size) else {
        return Ok(false);
    };

    for id in node.content.iter().flatten() {
        let ie = repo.get_index_entry(BlobType::Data, id)?;
        let length = ie.data_length();
        if !id.blob_matches_reader(length as usize, &mut open_file) {
            return Ok(false);
        }
    }
    Ok(true)
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
    mut tree_streamer1: impl Iterator<Item = RusticResult<(PathBuf, Node)>>,
    mut tree_streamer2: impl Iterator<Item = RusticResult<(PathBuf, Node)>>,
    no_content: bool,
    file_identical: impl Fn(&Path, &Node, &Node) -> Result<bool>,
    metadata: bool,
) -> Result<()> {
    let mut item1 = tree_streamer1.next().transpose()?;
    let mut item2 = tree_streamer2.next().transpose()?;

    loop {
        match (&item1, &item2) {
            (None, None) => break,
            (Some(i1), None) => {
                println!("-    {:?}", i1.0);
                item1 = tree_streamer1.next().transpose()?;
            }
            (None, Some(i2)) => {
                println!("+    {:?}", i2.0);
                item2 = tree_streamer2.next().transpose()?;
            }
            (Some(i1), Some(i2)) if i1.0 < i2.0 => {
                println!("-    {:?}", i1.0);
                item1 = tree_streamer1.next().transpose()?;
            }
            (Some(i1), Some(i2)) if i1.0 > i2.0 => {
                println!("+    {:?}", i2.0);
                item2 = tree_streamer2.next().transpose()?;
            }
            (Some(i1), Some(i2)) => {
                let path = &i1.0;
                let node1 = &i1.1;
                let node2 = &i2.1;
                match &node1.node_type {
                    tpe if tpe != &node2.node_type => println!("T    {path:?}"), // type was changed
                    NodeType::File if !no_content && !file_identical(path, node1, node2)? => {
                        println!("M    {path:?}");
                    }
                    NodeType::File if metadata && node1.meta != node2.meta => {
                        println!("U    {path:?}");
                    }
                    NodeType::Symlink { .. } => {
                        if node1.node_type.to_link() != node1.node_type.to_link() {
                            println!("U    {path:?}");
                        }
                    }
                    _ => {} // no difference to show
                }
                item1 = tree_streamer1.next().transpose()?;
                item2 = tree_streamer2.next().transpose()?;
            }
        }
    }

    Ok(())
}
