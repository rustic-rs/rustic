use anyhow::Result;
use chrono::{Duration, Local};
use clap::Parser;

use super::progress_counter;
use crate::backend::{DecryptFullBackend, FileType};
use crate::repo::{DeleteOption, SnapshotFile, SnapshotFilter, StringList};

#[derive(Parser)]
pub(super) struct Opts {
    #[clap(flatten)]
    filter: SnapshotFilter,

    /// Tags to add (can be specified multiple times)
    #[clap(long, value_name = "TAG[,TAG,..]", conflicts_with = "remove")]
    add: Vec<StringList>,

    /// Tags to remove (can be specified multiple times)
    #[clap(long, value_name = "TAG[,TAG,..]")]
    remove: Vec<StringList>,

    /// Tag list to set (can be specified multiple times)
    #[clap(long, value_name = "TAG[,TAG,..]", conflicts_with = "remove")]
    set: Vec<StringList>,

    /// Remove any delete mark
    #[clap(long, conflicts_with_all = &["set-delete-never", "set-delete-after"])]
    remove_delete: bool,

    /// Mark snapshot as uneraseable
    #[clap(long, conflicts_with = "set-delete-after")]
    set_delete_never: bool,

    /// Mark snapshot to be deleted after given duration (e.g. 10d)
    #[clap(long, value_name = "DURATION")]
    set_delete_after: Option<humantime::Duration>,

    /// don't change any snapshot, only show which would be modified
    #[clap(long, short = 'n')]
    dry_run: bool,

    /// Snapshots to change tags
    #[clap(value_name = "ID")]
    ids: Vec<String>,
}

pub(super) async fn execute(be: &impl DecryptFullBackend, opts: Opts) -> Result<()> {
    let snapshots = match opts.ids.is_empty() {
        true => SnapshotFile::all_from_backend(be, &opts.filter).await?,
        false => SnapshotFile::from_ids(be, &opts.ids).await?,
    };

    let delete = match (
        opts.remove_delete,
        opts.set_delete_never,
        opts.set_delete_after,
    ) {
        (true, _, _) => Some(DeleteOption::NotSet),
        (_, true, _) => Some(DeleteOption::Never),
        (_, _, Some(d)) => Some(DeleteOption::After(Local::now() + Duration::from_std(*d)?)),
        (false, false, None) => None,
    };

    let snapshots: Vec<_> = snapshots
        .into_iter()
        .filter_map(|sn| modify_sn(sn, &opts, &delete))
        .collect();

    let old_snap_ids: Vec<_> = snapshots.iter().map(|sn| sn.id).collect();

    match (old_snap_ids.is_empty(), opts.dry_run) {
        (true, _) => println!("no snapshot changed."),
        (false, true) => println!(
            "would have modified the following snapshots:\n {:?}",
            old_snap_ids
        ),
        (false, false) => {
            println!("saving new snapshots...");
            be.save_list(snapshots, progress_counter()).await?;

            println!("deleting old snapshots...");
            be.delete_list(FileType::Snapshot, true, old_snap_ids, progress_counter())
                .await?;
        }
    }
    Ok(())
}

fn modify_sn(
    mut sn: SnapshotFile,
    opts: &Opts,
    delete: &Option<DeleteOption>,
) -> Option<SnapshotFile> {
    let mut changed = false;

    if !opts.set.is_empty() {
        changed |= sn.set_tags(opts.set.clone());
    }
    changed |= sn.add_tags(opts.add.clone());
    changed |= sn.remove_tags(opts.remove.clone());

    if let Some(delete) = delete {
        if &sn.delete != delete {
            sn.delete = delete.clone();
            changed = true;
        }
    }

    changed.then(|| sn)
}
