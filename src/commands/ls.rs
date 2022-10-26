use anyhow::Result;
use clap::Parser;
use std::path::Path;

use super::progress_counter;
use crate::backend::DecryptReadBackend;
use crate::blob::{NodeStreamer, Tree};
use crate::index::IndexBackend;
use crate::repo::SnapshotFile;

#[derive(Parser)]
pub(super) struct Opts {
    /// Snapshot/path to list
    #[clap(value_name = "SNAPSHOT[:PATH]")]
    snap: String,
}

pub(super) fn execute(be: &(impl DecryptReadBackend + Unpin), opts: Opts) -> Result<()> {
    let (id, path) = opts.snap.split_once(':').unwrap_or((&opts.snap, ""));
    let snap = SnapshotFile::from_str(be, id, |_| true, progress_counter(""))?;
    let index = IndexBackend::new(be, progress_counter(""))?;
    let tree = Tree::subtree_id(&index, snap.tree, Path::new(path))?;

    for item in NodeStreamer::new(index, tree)? {
        let (path, _) = item?;
        println!("{:?} ", path);
    }

    Ok(())
}
