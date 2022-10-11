use std::collections::HashMap;

use anyhow::Result;
use bytes::Bytes;
use clap::Parser;
use futures::{stream, StreamExt, TryStreamExt};
use indicatif::ProgressBar;
use log::*;
use tokio::task::spawn_blocking;
use zstd::stream::decode_all;

use super::{progress_bytes, progress_counter};
use crate::backend::{Cache, DecryptReadBackend, FileType, ReadBackend};
use crate::blob::{BlobType, NodeType, TreeStreamerOnce};
use crate::commands::helpers::progress_spinner;
use crate::crypto::hash;
use crate::id::Id;
use crate::index::{IndexBackend, IndexCollector, IndexType, IndexedBackend};
use crate::repo::{
    IndexFile, IndexPack, PackHeader, PackHeaderLength, PackHeaderRef, SnapshotFile,
};

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
                // TODO: Make concurrency (20) customizable
                check_cache_files(20, cache, raw_be, file_type, p).await?;
            }
        }
    }

    if let Some(hot_be) = hot_be {
        for file_type in [FileType::Snapshot, FileType::Index] {
            check_hot_files(raw_be, hot_be, file_type).await?;
        }
    }

    let index_collector = check_packs(be, hot_be, opts.read_data).await?;

    if !opts.trust_cache {
        if let Some(cache) = &cache {
            let p = progress_bytes("checking packs in cache...");
            // TODO: Make concurrency (5) customizable
            check_cache_files(5, cache, raw_be, FileType::Pack, p).await?;
        }
    }

    let index_be = IndexBackend::new_from_index(be, index_collector.into_index());

    check_snapshots(&index_be).await?;

    if opts.read_data {
        let p = progress_counter("reading pack data...");
        stream::iter(index_be.into_index().into_iter().map(|pack| {
            let be = be.clone();
            let p = p.clone();
            (pack, be, p)
        }))
        // TODO: Make concurrency (4) customizable
        .for_each_concurrent(4, |(pack, be, p)| async move {
            let id = pack.id;
            let data = be.read_full(FileType::Pack, &id).await.unwrap();
            spawn_blocking(move || {
                match check_pack(&be, pack, data) {
                    Ok(()) => {}
                    Err(err) => error!("Error reading pack {id} : {err}",),
                }
                p.inc(1);
            })
            .await
            .unwrap()
        })
        .await;
        p.finish();
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
            None => error!("hot file Type: {file_type:?}, Id: {id} does not exist in repo",),
            Some(size) if size != size_hot => {
                error!("Type: {file_type:?}, Id: {id}: hot size: {size_hot}, actual size: {size}",)
            }
            _ => {} //everything ok
        }
    }

    for (id, _) in files {
        error!("hot file Type: {file_type:?}, Id: {id} is missing!",);
    }
    p.finish();

    Ok(())
}

async fn check_cache_files(
    concurrency: usize,
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
    .for_each_concurrent(concurrency, |((id, size), cache, be, p)| async move {
        // Read file from cache and from backend and compare
        match (
            cache.read_full(file_type, &id).await,
            be.read_full(file_type, &id).await,
        ) {
            (Err(err), _) => {
                error!("Error reading cached file Type: {file_type:?}, Id: {id} : {err}",)
            }
            (_, Err(err)) => error!("Error reading file Type: {file_type:?}, Id: {id} : {err}",),
            (Ok(data_cached), Ok(data)) if data_cached != data => {
                error!("Cached file Type: {file_type:?}, Id: {id} is not identical to backend!",)
            }
            (Ok(_), Ok(_)) => {} // everything ok
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
    read_data: bool,
) -> Result<IndexCollector> {
    let mut packs = HashMap::new();
    let mut tree_packs = HashMap::new();
    let mut index_collector = IndexCollector::new(if read_data {
        IndexType::Full
    } else {
        IndexType::FullTrees
    });

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
                    "pack {}: blob {} blob type does not match: type: {:?}, expected: {:?}",
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
            None => warn!("pack {id} not referenced in index. Can be a parallel backup job. To repair: 'rustic repair index'."),
            Some(index_size) if index_size != size => {
                error!("pack {id}: size computed by index: {index_size}, actual size: {size}. To repair: 'rustic repair index'.")
            }
            _ => {} //everything ok
        }
    }

    for (id, _) in packs {
        error!("pack {id} is referenced by the index but not present! To repair: 'rustic repair index'.",);
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
                                "file {:?} blob {} is missing in index",
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

fn check_pack(be: &impl DecryptReadBackend, index_pack: IndexPack, mut data: Bytes) -> Result<()> {
    let id = index_pack.id;
    let size = index_pack.pack_size();
    if data.len() != size as usize {
        error!(
            "pack {id}: data size does not match expected size. Read: {} bytes, expected: {size} bytes",
            data.len()
        );
        return Ok(());
    }

    let comp_id = hash(&data);
    if id != comp_id {
        error!("pack {id}: Hash mismatch. Computed hash: {comp_id}");
        return Ok(());
    }

    // check header length
    let header_len = PackHeaderRef::from_index_pack(&index_pack).size();
    let pack_header_len = PackHeaderLength::from_binary(&data.split_off(data.len() - 4))?.to_u32();
    if pack_header_len != header_len {
        error!("pack {id}: Header length in pack file doesn't match index. In pack: {pack_header_len}, calculated: {header_len}");
        return Ok(());
    }

    // check header
    let header = be.decrypt(&data.split_off(data.len() - header_len as usize))?;

    let pack_blobs = PackHeader::from_binary(&header)?.into_blobs();
    let mut blobs = index_pack.blobs;
    blobs.sort_unstable_by_key(|b| b.offset);
    if pack_blobs != blobs {
        error!("pack {id}: Header from pack file does not match the index");
        debug!("pack file header: {pack_blobs:?}");
        debug!("index: {:?}", blobs);
        return Ok(());
    }

    // check blobs
    for blob in blobs {
        let blob_id = blob.id;
        let mut blob_data = be.decrypt(&data.split_to(blob.length as usize))?;

        // TODO: this is identical to backend/decrypt.rs; unify these two parts!
        if let Some(length) = blob.uncompressed_length {
            blob_data = decode_all(&*blob_data).unwrap();
            if blob_data.len() != length.get() as usize {
                error!("pack {id}, blob {blob_id}: Actual uncompressed length does not fit saved uncompressed length");
                return Ok(());
            }
        }

        let comp_id = hash(&blob_data);
        if blob.id != comp_id {
            error!("pack {id}, blob {blob_id}: Hash mismatch. Computed hash: {comp_id}");
            return Ok(());
        }
    }

    Ok(())
}
