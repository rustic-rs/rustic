use anyhow::Result;
use clap::Parser;
use futures::StreamExt;
use vlog::*;

use super::progress_counter;
use crate::backend::DecryptReadBackend;
use crate::blob::{NodeType, TreeStreamer};
use crate::index::IndexBackend;
use crate::repo::SnapshotFile;

#[derive(Parser)]
pub(super) struct Opts {
    /// reference snapshot
    id1: String,

    /// new snapshot
    id2: String,
}

pub(super) async fn execute(be: &(impl DecryptReadBackend + Unpin), opts: Opts) -> Result<()> {
    v1!("getting snapshots...");
    let snaps = SnapshotFile::from_ids(be, &[opts.id1, opts.id2]).await?;

    let snap1 = &snaps[0];
    let snap2 = &snaps[1];

    let index = IndexBackend::new(be, progress_counter()).await?;

    let mut tree_streamer1 = TreeStreamer::new(index.clone(), vec![snap1.tree], false).await?;
    let mut tree_streamer2 = TreeStreamer::new(index, vec![snap2.tree], false).await?;

    let mut item1 = tree_streamer1.next().await.transpose()?;
    let mut item2 = tree_streamer2.next().await.transpose()?;

    loop {
        match (&item1, &item2) {
            (None, None) => break,
            (Some(i1), None) => {
                println!("-    {:?}", i1.0);
                item1 = tree_streamer1.next().await.transpose()?;
            }
            (None, Some(i2)) => {
                println!("+    {:?}", i2.0);
                item2 = tree_streamer2.next().await.transpose()?;
            }
            (Some(i1), Some(i2)) if i1.0 < i2.0 => {
                println!("-    {:?}", i1.0);
                item1 = tree_streamer1.next().await.transpose()?;
            }
            (Some(i1), Some(i2)) if i1.0 > i2.0 => {
                println!("+    {:?}", i2.0);
                item2 = tree_streamer2.next().await.transpose()?;
            }
            (Some(i1), Some(i2)) => {
                let path = &i1.0;
                let node1 = &i1.1;
                let node2 = &i2.1;
                match node1.node_type() {
                    tpe if tpe != node2.node_type() => println!("M    {:?}", path), // type was changed
                    NodeType::File if node1.content() != node2.content() => {
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
                item1 = tree_streamer1.next().await.transpose()?;
                item2 = tree_streamer2.next().await.transpose()?;
            }
        }
    }

    Ok(())
}
