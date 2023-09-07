//! `check` subcommand
use std::collections::HashMap;

use bytes::Bytes;
use derive_setters::Setters;
use itertools::Itertools;
use log::{debug, error, warn};
use rayon::prelude::{IntoParallelIterator, ParallelBridge, ParallelIterator};
use zstd::stream::decode_all;

use crate::{
    backend::{cache::Cache, decrypt::DecryptReadBackend, node::NodeType, FileType, ReadBackend},
    blob::{tree::TreeStreamerOnce, BlobType},
    crypto::hasher::hash,
    error::RusticResult,
    id::Id,
    index::{
        binarysorted::{IndexCollector, IndexType},
        IndexBackend, IndexedBackend,
    },
    progress::Progress,
    progress::ProgressBars,
    repofile::{IndexFile, IndexPack, PackHeader, PackHeaderLength, PackHeaderRef, SnapshotFile},
    repository::{Open, Repository},
};

#[cfg_attr(feature = "clap", derive(clap::Parser))]
#[derive(Clone, Copy, Debug, Default, Setters)]
#[setters(into)]
/// Options for the `check` command
pub struct CheckOptions {
    /// Don't verify the data saved in the cache
    #[cfg_attr(feature = "clap", clap(long, conflicts_with = "no_cache"))]
    pub trust_cache: bool,

    /// Read all data blobs
    #[cfg_attr(feature = "clap", clap(long))]
    pub read_data: bool,
}

impl CheckOptions {
    /// Runs the `check` command
    ///
    /// # Type Parameters
    ///
    /// * `P` - The progress bar type.
    /// * `S` - The state the repository is in.
    ///
    /// # Arguments
    ///
    /// * `repo` - The repository to check
    ///
    /// # Errors
    ///
    /// If the repository is corrupted
    pub(crate) fn run<P: ProgressBars, S: Open>(self, repo: &Repository<P, S>) -> RusticResult<()> {
        let be = repo.dbe();
        let cache = repo.cache();
        let hot_be = &repo.be_hot;
        let raw_be = &repo.be;
        let pb = &repo.pb;
        if !self.trust_cache {
            if let Some(cache) = &cache {
                for file_type in [FileType::Snapshot, FileType::Index] {
                    // list files in order to clean up the cache
                    //
                    // This lists files here and later when reading index / checking snapshots
                    // TODO: Only list the files once...
                    _ = be.list_with_size(file_type)?;

                    let p = pb.progress_bytes(format!("checking {file_type:?} in cache..."));
                    // TODO: Make concurrency (20) customizable
                    check_cache_files(20, cache, raw_be, file_type, &p)?;
                }
            }
        }

        if let Some(hot_be) = hot_be {
            for file_type in [FileType::Snapshot, FileType::Index] {
                check_hot_files(raw_be, hot_be, file_type, pb)?;
            }
        }

        let index_collector = check_packs(be, hot_be, self.read_data, pb)?;

        if let Some(cache) = &cache {
            let p = pb.progress_spinner("cleaning up packs from cache...");
            cache.remove_not_in_list(FileType::Pack, index_collector.tree_packs())?;
            p.finish();

            if !self.trust_cache {
                let p = pb.progress_bytes("checking packs in cache...");
                // TODO: Make concurrency (5) customizable
                check_cache_files(5, cache, raw_be, FileType::Pack, &p)?;
            }
        }

        let total_pack_size: u64 = index_collector
            .data_packs()
            .iter()
            .map(|(_, size)| u64::from(*size))
            .sum::<u64>()
            + index_collector
                .tree_packs()
                .iter()
                .map(|(_, size)| u64::from(*size))
                .sum::<u64>();

        let index_be = IndexBackend::new_from_index(be, index_collector.into_index());

        check_snapshots(&index_be, pb)?;

        if self.read_data {
            let p = pb.progress_bytes("reading pack data...");
            p.set_length(total_pack_size);

            index_be
                .into_index()
                .into_iter()
                .par_bridge()
                .for_each_with((be.clone(), p.clone()), |(be, p), pack| {
                    let id = pack.id;
                    let data = be.read_full(FileType::Pack, &id).unwrap();
                    match check_pack(be, pack, data, p) {
                        Ok(()) => {}
                        Err(err) => error!("Error reading pack {id} : {err}",),
                    }
                });
            p.finish();
        }
        Ok(())
    }
}

/// Checks if all files in the backend are also in the hot backend
///
/// # Arguments
///
/// * `be` - The backend to check
/// * `be_hot` - The hot backend to check
/// * `file_type` - The type of the files to check
/// * `pb` - The progress bar to use
///
/// # Errors
///
/// If a file is missing or has a different size
fn check_hot_files(
    be: &impl ReadBackend,
    be_hot: &impl ReadBackend,
    file_type: FileType,
    pb: &impl ProgressBars,
) -> RusticResult<()> {
    let p = pb.progress_spinner(format!("checking {file_type:?} in hot repo..."));
    let mut files = be
        .list_with_size(file_type)?
        .into_iter()
        .collect::<HashMap<_, _>>();

    let files_hot = be_hot.list_with_size(file_type)?;

    for (id, size_hot) in files_hot {
        match files.remove(&id) {
            None => error!("hot file Type: {file_type:?}, Id: {id} does not exist in repo"),
            Some(size) if size != size_hot => {
                // TODO: This should be an actual error not a log entry
                error!("Type: {file_type:?}, Id: {id}: hot size: {size_hot}, actual size: {size}");
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

/// Checks if all files in the cache are also in the backend
///
/// # Arguments
///
/// * `concurrency` - The number of threads to use
/// * `cache` - The cache to check
/// * `be` - The backend to check
/// * `file_type` - The type of the files to check
/// * `p` - The progress bar to use
///
/// # Errors
///
/// If a file is missing or has a different size
fn check_cache_files(
    _concurrency: usize,
    cache: &Cache,
    be: &impl ReadBackend,
    file_type: FileType,
    p: &impl Progress,
) -> RusticResult<()> {
    let files = cache.list_with_size(file_type)?;

    if files.is_empty() {
        return Ok(());
    }

    let total_size = files.values().map(|size| u64::from(*size)).sum();
    p.set_length(total_size);

    files
        .into_par_iter()
        .for_each_with((cache, be, p.clone()), |(cache, be, p), (id, size)| {
            // Read file from cache and from backend and compare
            match (
                cache.read_full(file_type, &id),
                be.read_full(file_type, &id),
            ) {
                (Err(err), _) => {
                    error!("Error reading cached file Type: {file_type:?}, Id: {id} : {err}");
                }
                (_, Err(err)) => {
                    error!("Error reading file Type: {file_type:?}, Id: {id} : {err}");
                }
                (Ok(data_cached), Ok(data)) if data_cached != data => {
                    error!(
                        "Cached file Type: {file_type:?}, Id: {id} is not identical to backend!"
                    );
                }
                (Ok(_), Ok(_)) => {} // everything ok
            }

            p.inc(u64::from(size));
        });

    p.finish();
    Ok(())
}

/// Check if packs correspond to index and are present in the backend
///
/// # Arguments
///
/// * `be` - The backend to check
/// * `hot_be` - The hot backend to check
/// * `read_data` - Whether to read the data of the packs
/// * `pb` - The progress bar to use
///
/// # Errors
///
/// If a pack is missing or has a different size
///
/// # Returns
///
/// The index collector
fn check_packs(
    be: &impl DecryptReadBackend,
    hot_be: &Option<impl ReadBackend>,
    read_data: bool,
    pb: &impl ProgressBars,
) -> RusticResult<IndexCollector> {
    let mut packs = HashMap::new();
    let mut tree_packs = HashMap::new();
    let mut index_collector = IndexCollector::new(if read_data {
        IndexType::Full
    } else {
        IndexType::DataIds
    });

    let mut process_pack = |p: IndexPack, check_time: bool| {
        let blob_type = p.blob_type();
        let pack_size = p.pack_size();
        _ = packs.insert(p.id, pack_size);
        if hot_be.is_some() && blob_type == BlobType::Tree {
            _ = tree_packs.insert(p.id, pack_size);
        }

        // Check if time is set _
        if check_time && p.time.is_none() {
            error!("pack {}: No time is set! Run prune to correct this!", p.id);
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

    let p = pb.progress_counter("reading index...");
    for index in be.stream_all::<IndexFile>(&p)? {
        let index = index?.1;
        index_collector.extend(index.packs.clone());
        for p in index.packs {
            process_pack(p, false);
        }
        for p in index.packs_to_delete {
            process_pack(p, true);
        }
    }

    p.finish();

    if let Some(hot_be) = hot_be {
        let p = pb.progress_spinner("listing packs in hot repo...");
        check_packs_list(hot_be, tree_packs)?;
        p.finish();
    }

    let p = pb.progress_spinner("listing packs...");
    check_packs_list(be, packs)?;
    p.finish();

    Ok(index_collector)
}

// TODO: Add documentation
/// Checks if all packs in the backend are also in the index
///
/// # Arguments
///
/// * `be` - The backend to check
/// * `packs` - The packs to check
///
/// # Errors
///
/// If a pack is missing or has a different size
fn check_packs_list(be: &impl ReadBackend, mut packs: HashMap<Id, u32>) -> RusticResult<()> {
    for (id, size) in be.list_with_size(FileType::Pack)? {
        match packs.remove(&id) {
            None => warn!("pack {id} not referenced in index. Can be a parallel backup job. To repair: 'rustic repair index'."),
            Some(index_size) if index_size != size => {
                error!("pack {id}: size computed by index: {index_size}, actual size: {size}. To repair: 'rustic repair index'.");
            }
            _ => {} //everything ok
        }
    }

    for (id, _) in packs {
        error!("pack {id} is referenced by the index but not present! To repair: 'rustic repair index'.",);
    }
    Ok(())
}

/// Check if all snapshots and contained trees can be loaded and contents exist in the index
///
/// # Arguments
///
/// * `index` - The index to check
/// * `pb` - The progress bar to use
///
/// # Errors
///
/// If a snapshot or tree is missing or has a different size
fn check_snapshots(index: &impl IndexedBackend, pb: &impl ProgressBars) -> RusticResult<()> {
    let p = pb.progress_counter("reading snapshots...");
    let snap_trees: Vec<_> = index
        .be()
        .stream_all::<SnapshotFile>(&p)?
        .iter()
        .map_ok(|(_, snap)| snap.tree)
        .try_collect()?;
    p.finish();

    let p = pb.progress_counter("checking trees...");
    let mut tree_streamer = TreeStreamerOnce::new(index.clone(), snap_trees, p)?;
    while let Some(item) = tree_streamer.next().transpose()? {
        let (path, tree) = item;
        for node in tree.nodes {
            match node.node_type {
                NodeType::File => node.content.as_ref().map_or_else(
                    || {
                        error!("file {:?} doesn't have a content", path.join(node.name()));
                    },
                    |content| {
                        for (i, id) in content.iter().enumerate() {
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
                    },
                ),

                NodeType::Dir => {
                    match node.subtree {
                        None => {
                            error!("dir {:?} subtree does not exist", path.join(node.name()));
                        }
                        Some(tree) if tree.is_null() => {
                            error!("dir {:?} subtree has null ID", path.join(node.name()));
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

/// Check if a pack is valid
///
/// # Arguments
///
/// * `be` - The backend to use
/// * `index_pack` - The pack to check
/// * `data` - The data of the pack
/// * `p` - The progress bar to use
///
/// # Errors
///
/// If the pack is invalid
///
/// # Panics
///
/// If zstd decompression fails.
fn check_pack(
    be: &impl DecryptReadBackend,
    index_pack: IndexPack,
    mut data: Bytes,
    p: &impl Progress,
) -> RusticResult<()> {
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
    p.inc(u64::from(header_len) + 4);

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
        p.inc(blob.length.into());
    }

    Ok(())
}
