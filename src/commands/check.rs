use std::collections::HashMap;

use anyhow::Result;
use clap::Parser;
use futures::TryStreamExt;
use vlog::*;

use super::progress_counter;
use crate::backend::{DecryptReadBackend, FileType};
use crate::blob::{NodeType, TreeStreamerOnce};
use crate::index::{IndexBackend, IndexCollector, IndexType, IndexedBackend};
use crate::repo::{IndexFile, IndexPack, SnapshotFile};

#[derive(Parser)]
pub(super) struct Opts {
    /// read all data blobs
    #[clap(long)]
    read_data: bool,
}

pub(super) async fn execute(be: &(impl DecryptReadBackend + Unpin), opts: Opts) -> Result<()> {
    v1!("checking packs in index and from pack list...");
    let index_collector = check_packs(be).await?;

    let be = IndexBackend::new_from_index(be, index_collector.into_index());

    v1!("checking snapshots and trees...");
    check_snapshots(&be).await?;

    if opts.read_data {
        unimplemented!()
    }

    Ok(())
}

// check if packs correspond to index
async fn check_packs(be: &impl DecryptReadBackend) -> Result<IndexCollector> {
    let mut packs = HashMap::new();
    let mut index_collector = IndexCollector::new(IndexType::FullTrees);

    let mut process_pack = |p: IndexPack| {
        packs.insert(p.id, p.pack_size());

        // check offsests in index
        let mut expected_offset: u32 = 0;
        let mut blobs = p.blobs;
        blobs.sort_unstable();
        for blob in blobs {
            if blob.offset != expected_offset {
                eprintln!(
                    "pack {}: blob {} offset in index: {}, expected: {}",
                    p.id, blob.id, blob.offset, expected_offset
                );
            }
            expected_offset += blob.length;
        }
    };

    v1!("- reading index...");
    let mut stream = be.stream_all::<IndexFile>(progress_counter()).await?;
    while let Some(index) = stream.try_next().await? {
        let index = index.1;
        index_collector.extend(index.packs.clone());
        for p in index.packs {
            process_pack(p);
        }
        for p in index.packs_to_delete {
            process_pack(p);
        }
    }

    v1!("- listing packs...");
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

    Ok(index_collector)
}

// check if all snapshots and contained trees can be loaded and contents exist in the index
async fn check_snapshots(index: &(impl IndexedBackend + Unpin)) -> Result<()> {
    let snap_trees: Vec<_> = index
        .be()
        .stream_all::<SnapshotFile>(progress_counter())
        .await?
        .map_ok(|(_, snap)| snap.tree)
        .try_collect()
        .await?;

    let mut tree_streamer = TreeStreamerOnce::new(index.clone(), snap_trees).await?;
    while let Some(item) = tree_streamer.try_next().await? {
        let (path, tree) = item;
        for node in tree.nodes() {
            match node.node_type() {
                NodeType::File => {
                    for (i, id) in node.content().iter().enumerate() {
                        if id.is_null() {
                            eprintln!("file {:?} blob {} has null ID", path.join(node.name()), i);
                        }

                        if !index.has_data(id) {
                            eprintln!(
                                "file {:?} blob {} is missig in index",
                                path.join(node.name()),
                                id
                            );
                        }
                    }
                }

                NodeType::Dir => {
                    match node.subtree() {
                        None => {
                            eprintln!("dir {:?} subtree does not exist", path.join(node.name()))
                        }
                        Some(tree) if tree.is_null() => {
                            eprintln!("dir {:?} subtree has null ID", path.join(node.name()))
                        }
                        _ => {} // subtree is ok
                    }
                }

                _ => {} // nothing to check
            }
        }
    }

    Ok(())
}
