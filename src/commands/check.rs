use std::collections::HashMap;

use anyhow::Result;
use clap::Parser;
use futures::{stream, StreamExt, TryStreamExt};
use indicatif::ProgressBar;
use log::*;

use super::{progress_bytes, progress_counter};
use crate::backend::{Cache, DecryptReadBackend, FileType, ReadBackend};
use crate::blob::{BlobType, NodeType, TreeStreamerOnce};
use crate::commands::helpers::progress_spinner;
use crate::id::Id;
use crate::index::{IndexBackend, IndexCollector, IndexType, IndexedBackend};
use crate::repo::{IndexFile, IndexPack, SnapshotFile};

#[derive(Parser)]
pub(super) struct Opts {
    /// Don't verify the data saved in the cache
    #[clap(long, conflicts_with = "no-cache")]
    trust_cache: bool,

    /// Read all data blobs
    #[clap(long)]
    read_data: bool,
}

pub(super) async fn execute(
    be: &(impl DecryptReadBackend + Unpin),
    cache: &Option<Cache>,
    hot_be: &Option<impl ReadBackend>,
    raw_be: &impl ReadBackend,
    opts: Opts,
) -> Result<()> {
    if !opts.trust_cache {
        if let Some(cache) = &cache {
            for file_type in [FileType::Snapshot, FileType::Index] {
                // list files in order to clean up the cache
                //
                // This lists files here and later when reading index / checking snapshots
                // TODO: Only list the files once...
                let _ = be.list_with_size(file_type).await?;

                let p = progress_bytes(format!("checking {} in cache...", file_type.name()));
                check_cache_files(cache, raw_be, file_type, p).await?;
            }
        }
    }

    if let Some(hot_be) = hot_be {
        for file_type in [FileType::Snapshot, FileType::Index] {
            check_hot_files(raw_be, hot_be, file_type).await?;
        }
    }

    let index_collector = check_packs(be, hot_be).await?;

    if !opts.trust_cache {
        if let Some(cache) = &cache {
            let p = progress_bytes("checking packs in cache...");
            check_cache_files(cache, raw_be, FileType::Pack, p).await?;
        }
    }

    let be = IndexBackend::new_from_index(be, index_collector.into_index());

    check_snapshots(&be).await?;

    if opts.read_data {
        unimplemented!()
    }

    Ok(())
}

async fn check_hot_files(
    be: &impl ReadBackend,
    be_hot: &impl ReadBackend,
    file_type: FileType,
) -> Result<()> {
    let p = progress_spinner(format!("checking {} in hot repo...", file_type.name()));
    let mut files = be
        .list_with_size(file_type)
        .await?
        .into_iter()
        .collect::<HashMap<_, _>>();

    let files_hot = be_hot.list_with_size(file_type).await?;

    for (id, size_hot) in files_hot {
        match files.remove(&id) {
            None => error!("hot file {} does not exist in repo", id.to_hex()),
            Some(size) if size != size_hot => error!(
                "file {}: hot size: {}, actual size: {}",
                id.to_hex(),
                size_hot,
                size
            ),
            _ => {} //everything ok
        }
    }

    for (id, _) in files {
        error!("hot file {} is missing!", id.to_hex());
    }
    p.finish();

    Ok(())
}

async fn check_cache_files(
    cache: &Cache,
    be: &impl ReadBackend,
    file_type: FileType,
    p: ProgressBar,
) -> Result<()> {
    let files = cache.list_with_size(file_type).await?;

    if files.is_empty() {
        return Ok(());
    }

    let total_size = files.iter().map(|(_, size)| *size as u64).sum();
    p.set_length(total_size);

    stream::iter(files.into_iter().map(|file| {
        let cache = cache.clone();
        let be = be.clone();
        let p = p.clone();
        (file, cache, be, p)
    }))
    .for_each_concurrent(5, |((id, size), cache, be, p)| async move {
        // Read file from cache and from backend and compare
        // TODO: Use (Async)Readers and compare using them!
        let data_cached = cache.read_full(file_type, &id).await.unwrap();
        let data = be.read_full(file_type, &id).await.unwrap();
        if data_cached != data {
            error!(
                "Cached file Type: {:?}, Id: {} is not identical to backend!",
                file_type, id
            );
        }
        p.inc(size as u64);
    })
    .await;

    p.finish();
    Ok(())
}

// check if packs correspond to index
async fn check_packs(
    be: &impl DecryptReadBackend,
    hot_be: &Option<impl ReadBackend>,
) -> Result<IndexCollector> {
    let mut packs = HashMap::new();
    let mut tree_packs = HashMap::new();
    let mut index_collector = IndexCollector::new(IndexType::FullTrees);

    let mut process_pack = |p: IndexPack| {
        let blob_type = p.blob_type();
        let pack_size = p.pack_size();
        packs.insert(p.id, pack_size);
        if hot_be.is_some() && blob_type == BlobType::Tree {
            tree_packs.insert(p.id, pack_size);
        }

        // check offsests in index
        let mut expected_offset: u32 = 0;
        let mut blobs = p.blobs;
        blobs.sort_unstable();
        for blob in blobs {
            if blob.tpe != blob_type {
                error!(
                    "pack {}: blob {} blob type does not match: {:?}, expected: {:?}",
                    p.id, blob.id, blob.tpe, blob_type
                );
            }

            if blob.offset != expected_offset {
                error!(
                    "pack {}: blob {} offset in index: {}, expected: {}",
                    p.id, blob.id, blob.offset, expected_offset
                );
            }
            expected_offset += blob.length;
        }
    };

    let p = progress_counter("reading index...");
    let mut stream = be.stream_all::<IndexFile>(p.clone()).await?;
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
    p.finish();

    if let Some(hot_be) = hot_be {
        let p = progress_spinner("listing packs in hot repo...");
        check_packs_list(hot_be, tree_packs).await?;
        p.finish();
    }

    let p = progress_spinner("listing packs...");
    check_packs_list(be, packs).await?;
    p.finish();

    Ok(index_collector)
}

async fn check_packs_list(be: &impl ReadBackend, mut packs: HashMap<Id, u32>) -> Result<()> {
    for (id, size) in be.list_with_size(FileType::Pack).await? {
        match packs.remove(&id) {
            None => error!("pack {} not referenced in index", id.to_hex()),
            Some(index_size) if index_size != size => error!(
                "pack {}: size computed by index: {}, actual size: {}",
                id.to_hex(),
                index_size,
                size
            ),
            _ => {} //everything ok
        }
    }

    for (id, _) in packs {
        error!(
            "pack {} is referenced by the index but not presend!",
            id.to_hex()
        );
    }
    Ok(())
}

// check if all snapshots and contained trees can be loaded and contents exist in the index
async fn check_snapshots(index: &(impl IndexedBackend + Unpin)) -> Result<()> {
    let p = progress_counter("reading snapshots...");
    let snap_trees: Vec<_> = index
        .be()
        .stream_all::<SnapshotFile>(p.clone())
        .await?
        .map_ok(|(_, snap)| snap.tree)
        .try_collect()
        .await?;
    p.finish();

    let p = progress_counter("checking trees...");
    let mut tree_streamer = TreeStreamerOnce::new(index.clone(), snap_trees, p).await?;
    while let Some(item) = tree_streamer.try_next().await? {
        let (path, tree) = item;
        for node in tree.nodes() {
            match node.node_type() {
                NodeType::File => {
                    for (i, id) in node.content().iter().enumerate() {
                        if id.is_null() {
                            error!("file {:?} blob {} has null ID", path.join(node.name()), i);
                        }

                        if !index.has_data(id) {
                            error!(
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
                            error!("dir {:?} subtree does not exist", path.join(node.name()))
                        }
                        Some(tree) if tree.is_null() => {
                            error!("dir {:?} subtree has null ID", path.join(node.name()))
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
