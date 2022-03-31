use std::collections::HashMap;

use anyhow::Result;
use clap::Parser;
use futures::{Stream, StreamExt};
use vlog::*;

use super::progress_counter;
use crate::backend::{DecryptReadBackend, FileType};
use crate::blob::{NodeType, TreeStreamer};
use crate::index::{IndexBackend, IndexedBackend};
use crate::repo::{IndexBlob, IndexFile, SnapshotFile};

#[derive(Parser)]
pub(super) struct Opts {
    /// read all data blobs
    #[clap(long)]
    read_data: bool,
}

pub(super) async fn execute(be: &(impl DecryptReadBackend + Unpin), opts: Opts) -> Result<()> {
    v1!("checking packs...");
    check_packs(be).await?;

    let be = IndexBackend::new(be, progress_counter()).await?;

    v1!("checking snapshots and trees...");
    check_snapshots(&be).await?;

    if opts.read_data {
        unimplemented!()
    }

    Ok(())
}

// calculate the pack size from the contained blobs
fn pack_size(blobs: &[IndexBlob]) -> u32 {
    let mut size = 4 + 32; // 4 + crypto overhead
    for blob in blobs {
        size += blob.length() + 37 // 37 = length of blob description
    }
    size
}

// check if packs correspond to index
async fn check_packs(be: &impl DecryptReadBackend) -> Result<()> {
    let mut packs = HashMap::new();

    // TODO: only read index files once
    let mut stream = be.stream_all::<IndexFile>(progress_counter()).await?;
    while let Some(index) = stream.next().await {
        let (_, index_packs) = index?.1.dissolve();
        for p in index_packs {
            packs.insert(*p.id(), pack_size(p.blobs()));
        }
    }

    for (id, size) in be.list_with_size(FileType::Pack).await? {
        match packs.remove(&id) {
            None => eprintln!("pack {} not contained in index", id.to_hex()),
            Some(index_size) if index_size != size => eprintln!(
                "pack {}: size computed by index: {}, actual size: {}",
                id.to_hex(),
                index_size,
                size
            ),
            _ => {} //everything ok
        }
    }

    for (id, _) in packs {
        eprintln!(
            "pack {} is referenced by the index but not presend!",
            id.to_hex()
        );
    }

    Ok(())
}

// check if all snapshots and contained trees can be loaded and contents exist in the index
async fn check_snapshots(index: &(impl IndexedBackend + Unpin)) -> Result<()> {
    let mut snap_trees = Vec::new();
    let mut stream = index
        .be()
        .stream_all::<SnapshotFile>(progress_counter())
        .await?;
    snap_trees.reserve(stream.size_hint().1.unwrap());
    while let Some(snap) = stream.next().await {
        snap_trees.push(snap?.1.tree);
    }

    let mut tree_streamer = TreeStreamer::new(index.clone(), snap_trees, true).await?;
    while let Some(item) = tree_streamer.next().await {
        let (path, node) = item?;
        match node.node_type() {
            NodeType::File => {
                for (i, id) in node.content().iter().enumerate() {
                    if id.is_null() {
                        eprintln!("file {:?} blob {} has null ID", path, i);
                    }

                    if !index.has_data(id) {
                        eprintln!("file {:?} blob {} is missig in index", path, id);
                    }
                }
            }

            NodeType::Dir => {
                match node.subtree() {
                    None => eprintln!("dir {:?} subtree does not exist", path),
                    Some(tree) if tree.is_null() => eprintln!("dir {:?} subtree has null ID", path),
                    _ => {} // subtree is ok
                }
            }

            _ => {} // nothing to check
        }
    }

    Ok(())
}
