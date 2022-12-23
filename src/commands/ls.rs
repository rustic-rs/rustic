use anyhow::Result;
use clap::Parser;
use std::path::Path;

use super::progress_counter;
use super::rustic_config::RusticConfig;
use crate::backend::DecryptReadBackend;
use crate::blob::{NodeStreamer, Tree};
use crate::index::IndexBackend;
use crate::repofile::{SnapshotFile, SnapshotFilter};

#[derive(Parser)]
pub(super) struct Opts {
    #[clap(flatten, help_heading = "SNAPSHOT FILTER OPTIONS (when using latest)")]
    filter: SnapshotFilter,

    /// Snapshot/path to list
    #[clap(value_name = "SNAPSHOT[:PATH]")]
    snap: String,
}

pub(super) fn execute(
    be: &impl DecryptReadBackend,
    mut opts: Opts,
    config_file: RusticConfig,
) -> Result<()> {
    config_file.merge_into("snapshot-filter", &mut opts.filter)?;

    let (id, path) = opts.snap.split_once(':').unwrap_or((&opts.snap, ""));
    let snap = SnapshotFile::from_str(be, id, |sn| sn.matches(&opts.filter), progress_counter(""))?;
    let index = IndexBackend::new(be, progress_counter(""))?;
    let node = Tree::node_from_path(&index, snap.tree, Path::new(path))?;

    for item in NodeStreamer::new(index, &node)? {
        let (path, _) = item?;
        println!("{:?} ", path);
    }

    Ok(())
}
