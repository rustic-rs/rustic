//! `smapshot` subcommand

use crate::{
    OpenRepository, ProgressBars, RusticResult, SnapshotFile, SnapshotGroup, SnapshotGroupCriterion,
};

pub(crate) fn get_snapshot_group<P: ProgressBars>(
    repo: &OpenRepository<P>,
    ids: &[String],
    group_by: SnapshotGroupCriterion,
    filter: impl FnMut(&SnapshotFile) -> bool,
) -> RusticResult<Vec<(SnapshotGroup, Vec<SnapshotFile>)>> {
    let pb = &repo.pb;
    let p = pb.progress_counter("getting snapshots...");
    let groups = match ids {
        [] => SnapshotFile::group_from_backend(&repo.dbe, filter, group_by, &p)?,
        [id] if id == "latest" => {
            SnapshotFile::group_from_backend(&repo.dbe, filter, group_by, &p)?
                .into_iter()
                .map(|(group, mut snaps)| {
                    snaps.sort_unstable();
                    let last_idx = snaps.len() - 1;
                    snaps.swap(0, last_idx);
                    snaps.truncate(1);
                    (group, snaps)
                })
                .collect::<Vec<_>>()
        }
        _ => {
            let item = (
                SnapshotGroup::default(),
                SnapshotFile::from_ids(&repo.dbe, ids, &p)?,
            );
            vec![item]
        }
    };

    Ok(groups)
}
