//! `merge` subcommand

use std::cmp::Ordering;

use chrono::Local;

use crate::{
    backend::{decrypt::DecryptWriteBackend, node::Node},
    blob::{
        packer::Packer,
        tree::{self, Tree},
        BlobType,
    },
    error::CommandErrorKind,
    error::RusticResult,
    id::Id,
    index::{indexer::Indexer, ReadIndex},
    progress::{Progress, ProgressBars},
    repofile::{PathList, SnapshotFile, SnapshotSummary},
    repository::{IndexedTree, Repository},
};

/// Merges the given snapshots into a new snapshot.
///
/// # Arguments
///
/// * `repo` - The repository to merge into
/// * `snapshots` - The snapshots to merge
/// * `cmp` - The comparison function for the trees
/// * `snap` - The snapshot to merge into
///
/// # Returns
///
/// The merged snapshot
pub(crate) fn merge_snapshots<P: ProgressBars, S: IndexedTree>(
    repo: &Repository<P, S>,
    snapshots: &[SnapshotFile],
    cmp: &impl Fn(&Node, &Node) -> Ordering,
    mut snap: SnapshotFile,
) -> RusticResult<SnapshotFile> {
    let now = Local::now();

    let paths = PathList::from_strings(snapshots.iter().flat_map(|snap| snap.paths.iter())).merge();
    snap.paths.set_paths(&paths.paths())?;

    // set snapshot time to time of latest snapshot to be merged
    snap.time = snapshots
        .iter()
        .max_by_key(|sn| sn.time)
        .map_or(now, |sn| sn.time);

    let mut summary = snap.summary.take().unwrap_or_default();
    summary.backup_start = Local::now();

    let trees: Vec<Id> = snapshots.iter().map(|sn| sn.tree).collect();
    snap.tree = merge_trees(repo, &trees, cmp, &mut summary)?;

    summary.finalize(now)?;
    snap.summary = Some(summary);

    snap.id = repo.dbe().save_file(&snap)?;
    Ok(snap)
}

/// Merges the given trees into a new tree.
///
/// # Type Parameters
///
/// * `P` - The progress bar type.
/// * `S` - The type of the indexed tree.
///
/// # Arguments
///
/// * `repo` - The repository to merge into
/// * `trees` - The trees to merge
/// * `cmp` - The comparison function for the trees
/// * `summary` - The summary to update
///
/// # Errors
///
/// * [`CommandErrorKind::ConversionToU64Failed`] - If the size of the tree is too large
///
/// # Returns
///
/// The merged tree
pub(crate) fn merge_trees<P: ProgressBars, S: IndexedTree>(
    repo: &Repository<P, S>,
    trees: &[Id],
    cmp: &impl Fn(&Node, &Node) -> Ordering,
    summary: &mut SnapshotSummary,
) -> RusticResult<Id> {
    let index = repo.index();
    let indexer = Indexer::new(repo.dbe().clone()).into_shared();
    let packer = Packer::new(
        repo.dbe().clone(),
        BlobType::Tree,
        indexer.clone(),
        repo.config(),
        index.total_size(BlobType::Tree),
    )?;
    let save = |tree: Tree| {
        let (chunk, new_id) = tree.serialize()?;
        let size = u64::try_from(chunk.len()).map_err(CommandErrorKind::ConversionToU64Failed)?;
        if !index.has_tree(&new_id) {
            packer.add(chunk.into(), new_id)?;
        }
        Ok((new_id, size))
    };

    let p = repo.pb.progress_spinner("merging snapshots...");
    let tree_merged = tree::merge_trees(index, trees, cmp, &save, summary)?;
    let stats = packer.finalize()?;
    indexer.write().unwrap().finalize()?;
    p.finish();

    stats.apply(summary, BlobType::Tree);

    Ok(tree_merged)
}
