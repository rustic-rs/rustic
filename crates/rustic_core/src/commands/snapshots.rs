//! `smapshot` subcommand

use crate::{
    repository::Open, ProgressBars, Repository, RusticResult, SnapshotFile, SnapshotGroup,
    SnapshotGroupCriterion,
};

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
