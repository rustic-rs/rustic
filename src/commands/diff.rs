//! `diff` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{commands::open_repository, status_err, Application, RUSTIC_APP};

use abscissa_core::{Command, Runnable, Shutdown};

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use rustic_core::{
    BlobType, IndexedFull, LocalDestination, LocalSource, LocalSourceFilterOptions,
    LocalSourceSaveOptions, Node, NodeType, ReadSourceEntry, Repository, RusticResult,
    TreeStreamerOptions,
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

        let repo = open_repository(&config)?.to_indexed()?;

        let (id1, path1) = arg_to_snap_path(&self.snap1, "");
        let (id2, path2) = arg_to_snap_path(&self.snap2, path1);

        _ = match (id1, id2) {
            (Some(id1), Some(id2)) => {
                // diff between two snapshots
                let snaps = repo.get_snapshots(&[id1.to_string(), id2.to_string()])?;

                let snap1 = &snaps[0];
                let snap2 = &snaps[1];

                let node1 = repo.node_from_snapshot_and_path(snap1, path1)?;
                let node2 = repo.node_from_snapshot_and_path(snap2, path2)?;

                diff(
                    repo.ls(&node1, &TreeStreamerOptions::default(), true)?,
                    repo.ls(&node2, &TreeStreamerOptions::default(), true)?,
                    self.no_content,
                    |_path, node1, node2| Ok(node1.content == node2.content),
                    self.metadata,
                )
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
                    repo.ls(&node1, &TreeStreamerOptions::default(), true)?,
                    src,
                    self.no_content,
                    |path, node1, _node2| identical_content_local(&local, &repo, path, node1),
                    self.metadata,
                )
            }
            (None, _) => {
                bail!("cannot use local path as first argument");
            }
        };

        Ok(())
    }
}

fn arg_to_snap_path<'a>(arg: &'a str, default_path: &'a str) -> (Option<&'a str>, &'a str) {
    match arg.split_once(':') {
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

fn identical_content_local<P, S: IndexedFull>(
    local: &LocalDestination,
    repo: &Repository<P, S>,
    path: &Path,
    node: &Node,
) -> Result<bool> {
    let Some(mut open_file) = local.get_matching_file(path, node.meta.size) else { return Ok(false) };

    for id in node.content.iter().flatten() {
        let ie = repo.get_index_entry(BlobType::Data, id)?;
        let length = ie.data_length();
        if !id.blob_matches_reader(length as usize, &mut open_file) {
            return Ok(false);
        }
    }
    Ok(true)
}

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
                    NodeType::Symlink { linktarget } => {
                        if let NodeType::Symlink {
                            linktarget: linktarget2,
                        } = &node2.node_type
                        {
                            if *linktarget != *linktarget2 {
                                println!("U    {path:?}");
                            }
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
