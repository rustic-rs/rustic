use anyhow::Result;
use chrono::{Duration, Local};
use clap::Parser;

use super::{progress_counter, Config};
use crate::backend::{DecryptWriteBackend, FileType};
use crate::id::Id;
use crate::repofile::{DeleteOption, SnapshotFile, StringList};
use crate::repository::OpenRepository;

#[derive(Parser)]
pub(super) struct Opts {
    /// Snapshots to change tags. If none is given, use filter to filter from all
    /// snapshots.
    #[clap(value_name = "ID")]
    ids: Vec<String>,

    /// Tags to add (can be specified multiple times)
    #[clap(
        long,
        value_name = "TAG[,TAG,..]",
        conflicts_with = "remove",
        help_heading = "Tag options"
    )]
    add: Vec<StringList>,

    /// Tags to remove (can be specified multiple times)
    #[clap(long, value_name = "TAG[,TAG,..]", help_heading = "Tag options")]
    remove: Vec<StringList>,

    /// Tag list to set (can be specified multiple times)
    #[clap(
        long,
        value_name = "TAG[,TAG,..]",
        conflicts_with = "remove",
        help_heading = "Tag options"
    )]
    set: Vec<StringList>,

    /// Remove any delete mark
    #[clap(
        long,
        conflicts_with_all = &["set_delete_never", "set_delete_after"], 
        help_heading = "Delete mark options"
    )]
    remove_delete: bool,

    /// Mark snapshot as uneraseable
    #[clap(
        long,
        conflicts_with = "set_delete_after",
        help_heading = "Delete mark options"
    )]
    set_delete_never: bool,

    /// Mark snapshot to be deleted after given duration (e.g. 10d)
    #[clap(long, value_name = "DURATION", help_heading = "Delete mark options")]
    set_delete_after: Option<humantime::Duration>,
}

pub(super) fn execute(repo: OpenRepository, config: Config, opts: Opts) -> Result<()> {
    let be = &repo.dbe;

    let snapshots = match opts.ids.is_empty() {
        true => SnapshotFile::all_from_backend(be, &config.snapshot_filter)?,
        false => SnapshotFile::from_ids(be, &opts.ids)?,
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

    let mut snapshots: Vec<_> = snapshots
        .into_iter()
        .filter_map(|sn| modify_sn(sn, &opts, &delete))
        .collect();
    let old_snap_ids: Vec<_> = snapshots.iter().map(|sn| sn.id).collect();
    // remove old ids from snapshots
    for snap in &mut snapshots {
        snap.id = Id::default();
    }

    match (old_snap_ids.is_empty(), config.global.dry_run) {
        (true, _) => println!("no snapshot changed."),
        (false, true) => {
            println!("would have modified the following snapshots:\n {old_snap_ids:?}");
        }
        (false, false) => {
            let p = progress_counter("saving new snapshots...");
            be.save_list(snapshots.iter(), p)?;

            let p = progress_counter("deleting old snapshots...");
            be.delete_list(FileType::Snapshot, true, old_snap_ids.iter(), p)?;
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

    changed.then_some(sn)
}
