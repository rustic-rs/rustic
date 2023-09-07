use std::collections::BTreeSet;

use log::trace;
use rayon::prelude::{IntoParallelRefIterator, ParallelBridge, ParallelIterator};

use crate::{
    backend::{decrypt::DecryptWriteBackend, node::NodeType},
    blob::{packer::Packer, tree::TreeStreamerOnce, BlobType},
    error::RusticResult,
    index::{indexer::Indexer, IndexedBackend, ReadIndex},
    progress::ProgressBars,
    repofile::SnapshotFile,
    repository::{IndexedFull, IndexedIds, IndexedTree, Open, Repository},
};

/// This struct enhances `[SnapshotFile]` with the attribute `relevant`
/// which indicates if the snapshot is relevant for copying.
#[derive(Debug)]
pub struct CopySnapshot {
    /// The snapshot
    pub sn: SnapshotFile,
    /// Whether it is relevant
    pub relevant: bool,
}

/// Copy the given snapshots to the destination repository.
///
/// # Type Parameters
///
/// * `Q` - The progress bar type.
/// * `R` - The type of the indexed tree.
/// * `P` - The progress bar type.
/// * `S` - The type of the indexed tree.
///
/// # Arguments
///
/// * `repo` - The repository to copy from
/// * `repo_dest` - The repository to copy to
/// * `snapshots` - The snapshots to copy
pub(crate) fn copy<'a, Q, R: IndexedFull, P: ProgressBars, S: IndexedIds>(
    repo: &Repository<Q, R>,
    repo_dest: &Repository<P, S>,
    snapshots: impl IntoIterator<Item = &'a SnapshotFile>,
) -> RusticResult<()> {
    let be_dest = repo_dest.dbe();
    let pb = &repo_dest.pb;

    let (snap_trees, snaps): (Vec<_>, Vec<_>) = snapshots
        .into_iter()
        .cloned()
        .map(|sn| (sn.tree, SnapshotFile::clear_ids(sn)))
        .unzip();

    let index = repo.index();
    let index_dest = repo_dest.index();
    let indexer = Indexer::new(be_dest.clone()).into_shared();

    let data_packer = Packer::new(
        be_dest.clone(),
        BlobType::Data,
        indexer.clone(),
        repo_dest.config(),
        index.total_size(BlobType::Data),
    )?;
    let tree_packer = Packer::new(
        be_dest.clone(),
        BlobType::Tree,
        indexer.clone(),
        repo_dest.config(),
        index.total_size(BlobType::Tree),
    )?;

    let p = pb.progress_counter("copying blobs in snapshots...");

    snap_trees
        .par_iter()
        .try_for_each(|id| -> RusticResult<_> {
            trace!("copy tree blob {id}");
            if !index_dest.has_tree(id) {
                let data = index.get_tree(id).unwrap().read_data(index.be())?;
                tree_packer.add(data, *id)?;
            }
            Ok(())
        })?;

    let tree_streamer = TreeStreamerOnce::new(index.clone(), snap_trees, p)?;
    tree_streamer
        .par_bridge()
        .try_for_each(|item| -> RusticResult<_> {
            let (_, tree) = item?;
            tree.nodes.par_iter().try_for_each(|node| {
                match node.node_type {
                    NodeType::File => {
                        node.content.par_iter().flatten().try_for_each(
                            |id| -> RusticResult<_> {
                                trace!("copy data blob {id}");
                                if !index_dest.has_data(id) {
                                    let data = index.get_data(id).unwrap().read_data(index.be())?;
                                    data_packer.add(data, *id)?;
                                }
                                Ok(())
                            },
                        )?;
                    }

                    NodeType::Dir => {
                        let id = node.subtree.unwrap();
                        trace!("copy tree blob {id}");
                        if !index_dest.has_tree(&id) {
                            let data = index.get_tree(&id).unwrap().read_data(index.be())?;
                            tree_packer.add(data, id)?;
                        }
                    }

                    _ => {} // nothing to copy
                }
                Ok(())
            })
        })?;

    _ = data_packer.finalize()?;
    _ = tree_packer.finalize()?;
    indexer.write().unwrap().finalize()?;

    let p = pb.progress_counter("saving snapshots...");
    be_dest.save_list(snaps.iter(), p)?;
    Ok(())
}

/// Filter out relevant snapshots from the given list of snapshots.
///
/// # Type Parameters
///
/// * `F` - The type of the filter.
/// * `P` - The progress bar type.
/// * `S` - The state of the repository.
///
/// # Arguments
///
/// * `snaps` - The snapshots to filter
/// * `dest_repo` - The destination repository
/// * `filter` - The filter to apply to the snapshots
///
/// # Returns
///
/// A list of snapshots with the attribute `relevant` set to `true` if the snapshot is relevant for copying.
pub(crate) fn relevant_snapshots<F, P: ProgressBars, S: Open>(
    snaps: &[SnapshotFile],
    dest_repo: &Repository<P, S>,
    filter: F,
) -> RusticResult<Vec<CopySnapshot>>
where
    F: FnMut(&SnapshotFile) -> bool,
{
    let p = dest_repo
        .pb
        .progress_counter("finding relevant snapshots...");
    // save snapshots in destination in BTreeSet, as we want to efficiently search within to filter out already existing snapshots before copying.
    let snapshots_dest: BTreeSet<_> = SnapshotFile::all_from_backend(dest_repo.dbe(), filter, &p)?
        .into_iter()
        .collect();

    let relevant = snaps
        .iter()
        .cloned()
        .map(|sn| CopySnapshot {
            relevant: !snapshots_dest.contains(&sn),
            sn,
        })
        .collect();

    Ok(relevant)
}
