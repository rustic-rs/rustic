use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::Parser;

use super::{progress_counter, RusticConfig};
use crate::backend::{LocalSource, LocalSourceOptions};
use crate::blob::{Node, NodeStreamer, NodeType, Tree};
use crate::commands::helpers::progress_spinner;
use crate::index::IndexBackend;
use crate::repofile::{SnapshotFile, SnapshotFilter};
use crate::repository::OpenRepository;

#[derive(Parser)]
pub(super) struct Opts {
    #[clap(flatten)]
    ignore_opts: LocalSourceOptions,

    #[clap(flatten, help_heading = "SNAPSHOT FILTER OPTIONS (when using latest)")]
    filter: SnapshotFilter,

    /// Reference snapshot/path
    #[clap(value_name = "SNAPSHOT1[:PATH1]")]
    snap1: String,

    /// New snapshot/path or local path [default for PATH2: PATH1]
    #[clap(value_name = "SNAPSHOT2[:PATH2]|PATH2")]
    snap2: String,
}

pub(super) fn execute(
    repo: OpenRepository,
    mut opts: Opts,
    config_file: RusticConfig,
) -> Result<()> {
    let be = &repo.dbe;
    let (id1, path1) = arg_to_snap_path(&opts.snap1, "");
    let (id2, path2) = arg_to_snap_path(&opts.snap2, path1);

    match (id1, id2) {
        (Some(id1), Some(id2)) => {
            // diff between two snapshots
            let p = progress_spinner("getting snapshots...");
            let snaps = SnapshotFile::from_ids(be, &[id1.to_string(), id2.to_string()])?;
            p.finish();

            let snap1 = &snaps[0];
            let snap2 = &snaps[1];

            let index = IndexBackend::new(be, progress_counter(""))?;
            let node1 = Tree::node_from_path(&index, snap1.tree, Path::new(path1))?;
            let node2 = Tree::node_from_path(&index, snap2.tree, Path::new(path2))?;

            diff(
                NodeStreamer::new(index.clone(), &node1)?,
                NodeStreamer::new(index, &node2)?,
                true,
            )
        }
        (Some(id1), None) => {
            // diff between snapshot and local path
            config_file.merge_into("snapshot-filter", &mut opts.filter)?;

            let p = progress_spinner("getting snapshot...");
            let snap1 = SnapshotFile::from_str(be, id1, |sn| sn.matches(&opts.filter), p.clone())?;
            p.finish();

            let index = IndexBackend::new(be, progress_counter(""))?;
            let node1 = Tree::node_from_path(&index, snap1.tree, Path::new(path1))?;
            let path2 = PathBuf::from(path2);
            let is_dir = path2
                .metadata()
                .with_context(|| format!("Error accessing {path2:?}"))?
                .is_dir();
            let src = LocalSource::new(opts.ignore_opts, path2.clone())?.map(|item| {
                let (path, node) = item?;
                let path = if is_dir {
                    // remove given path prefix for dirs as local path
                    path.strip_prefix(&path2)?.to_path_buf()
                } else {
                    // ensure that we really get the filename if local path is a file
                    path2.file_name().unwrap().into()
                };
                Ok((path, node))
            });

            diff(NodeStreamer::new(index, &node1)?, src, false)
        }
        (None, _) => bail!("cannot use local path as first argument"),
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

fn diff(
    mut tree_streamer1: impl Iterator<Item = Result<(PathBuf, Node)>>,
    mut tree_streamer2: impl Iterator<Item = Result<(PathBuf, Node)>>,
    check_content: bool,
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
                match node1.node_type() {
                    tpe if tpe != node2.node_type() => println!("M    {:?}", path), // type was changed
                    NodeType::File if node1.meta() != node2.meta() => {
                        println!("M    {:?}", path)
                    }
                    NodeType::File if check_content && node1.content() != node2.content() => {
                        println!("M    {:?}", path)
                    }
                    NodeType::Symlink { linktarget } => {
                        if let NodeType::Symlink {
                            linktarget: linktarget2,
                        } = node2.node_type()
                        {
                            if *linktarget != *linktarget2 {
                                println!("M    {:?}", path)
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
