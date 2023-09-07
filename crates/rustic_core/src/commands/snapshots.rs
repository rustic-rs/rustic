//! `smapshot` subcommand

use crate::{
    error::RusticResult,
    progress::ProgressBars,
    repofile::snapshotfile::{SnapshotGroup, SnapshotGroupCriterion},
    repofile::SnapshotFile,
    repository::{Open, Repository},
};

/// Get the snapshots from the repository.
///
/// # Type Parameters
///
/// * `P` - The progress bar type.
/// * `S` - The state the repository is in.
///
/// # Arguments
///
/// * `repo` - The repository to get the snapshots from.
/// * `ids` - The ids of the snapshots to get.
/// * `group_by` - The criterion to group the snapshots by.
/// * `filter` - The filter to apply to the snapshots.
///
/// # Returns
///
/// The snapshots grouped by the given criterion.
pub(crate) fn get_snapshot_group<P: ProgressBars, S: Open>(
    repo: &Repository<P, S>,
    ids: &[String],
    group_by: SnapshotGroupCriterion,
    filter: impl FnMut(&SnapshotFile) -> bool,
) -> RusticResult<Vec<(SnapshotGroup, Vec<SnapshotFile>)>> {
    let pb = &repo.pb;
    let dbe = repo.dbe();
    let p = pb.progress_counter("getting snapshots...");
    let groups = match ids {
        [] => SnapshotFile::group_from_backend(dbe, filter, group_by, &p)?,
        [id] if id == "latest" => SnapshotFile::group_from_backend(dbe, filter, group_by, &p)?
            .into_iter()
            .map(|(group, mut snaps)| {
                snaps.sort_unstable();
                let last_idx = snaps.len() - 1;
                snaps.swap(0, last_idx);
                snaps.truncate(1);
                (group, snaps)
            })
            .collect::<Vec<_>>(),
        _ => {
            let item = (
                SnapshotGroup::default(),
                SnapshotFile::from_ids(dbe, ids, &p)?,
            );
            vec![item]
        }
    };

    Ok(groups)
}
