use anyhow::Result;
use clap::Parser;
use futures::StreamExt;
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

pub(super) async fn execute(be: &(impl DecryptReadBackend + Unpin), opts: Opts) -> Result<()> {
    let (id, path) = opts.snap.split_once(':').unwrap_or((&opts.snap, ""));
    let snap = SnapshotFile::from_str(be, id, |_| true, progress_counter(""))?;
    let index = IndexBackend::new(be, progress_counter(""))?;
    let tree = Tree::subtree_id(&index, snap.tree, Path::new(path))?;

    let mut tree_streamer = NodeStreamer::new(index, tree)?;
    while let Some(item) = tree_streamer.next().await {
        let (path, _) = item?;
        println!("{:?} ", path);
    }

    Ok(())
}
