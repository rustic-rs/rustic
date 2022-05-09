use anyhow::Result;
use clap::Parser;
use futures::StreamExt;

use super::progress_counter;
use crate::backend::DecryptReadBackend;
use crate::blob::NodeStreamer;
use crate::index::IndexBackend;
use crate::repo::SnapshotFile;

#[derive(Parser)]
pub(super) struct Opts {
    /// snapshot to ls
    id: String,
}

pub(super) async fn execute(be: &(impl DecryptReadBackend + Unpin), opts: Opts) -> Result<()> {
    let snap = SnapshotFile::from_str(be, &opts.id, |_| true, progress_counter()).await?;
    let index = IndexBackend::new(be, progress_counter()).await?;

    let mut tree_streamer = NodeStreamer::new(index, snap.tree).await?;
    while let Some(item) = tree_streamer.next().await {
        let (path, _) = item?;
        println!("{:?} ", path);
    }

    Ok(())
}
