use anyhow::Result;
use clap::Parser;
use std::path::Path;

use super::progress_counter;
use super::Config;
use crate::blob::{NodeStreamer, Tree, TreeStreamerOptions};
use crate::index::IndexBackend;
use crate::repofile::SnapshotFile;
use crate::repository::OpenRepository;

#[derive(Parser)]
pub(super) struct Opts {
    /// Snapshot/path to list
    #[clap(value_name = "SNAPSHOT[:PATH]")]
    snap: String,

    /// recursively list the dir (default when no PATH is given)
    #[clap(long)]
    recursive: bool,

    #[clap(flatten)]
    streamer_opts: TreeStreamerOptions,
}

pub(super) fn execute(repo: OpenRepository, config: Config, opts: Opts) -> Result<()> {
    let be = &repo.dbe;
    let mut recursive = opts.recursive;

    let (id, path) = opts.snap.split_once(':').unwrap_or_else(|| {
        recursive = true;
        (&opts.snap, "")
    });
    let snap = SnapshotFile::from_str(
        be,
        id,
        |sn| sn.matches(&config.snapshot_filter),
        progress_counter(""),
    )?;
    let index = IndexBackend::new(be, progress_counter(""))?;
    let node = Tree::node_from_path(&index, snap.tree, Path::new(path))?;

    if recursive {
        for item in NodeStreamer::new_with_glob(index, &node, opts.streamer_opts)? {
            let (path, _) = item?;
            println!("{path:?} ");
        }
    } else if node.is_dir() {
        let tree = Tree::from_backend(&index, node.subtree.unwrap())?.nodes;
        for node in tree {
            println!("{:?} ", node.name());
        }
    } else {
        println!("{:?} ", node.name());
    }

    Ok(())
}
