use anyhow::Result;
use clap::Parser;

use super::progress_counter;
use crate::backend::{DecryptFullBackend, FileType};
use crate::repo::{SnapshotFile, SnapshotFilter, StringList};

#[derive(Parser)]
pub(super) struct Opts {
    #[clap(flatten)]
    filter: SnapshotFilter,

    /// Tags to add add (can be specified multiple times)
    #[clap(long, value_name = "TAG[,TAG,..]", conflicts_with = "remove")]
    add: Vec<StringList>,

    /// Tags to remove (can be specified multiple times)
    #[clap(long, value_name = "TAG[,TAG,..]")]
    remove: Vec<StringList>,

    /// Tag list to set (can be specified multiple times)
    #[clap(long, value_name = "TAG[,TAG,..]", conflicts_with = "remove")]
    set: Vec<StringList>,

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

    let snapshots: Vec<_> = snapshots
        .into_iter()
        .filter_map(|sn| modify_sn(sn, &opts))
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
            be.delete_list(FileType::Snapshot, old_snap_ids, progress_counter())
                .await?;
        }
    }
    Ok(())
}

fn modify_sn(mut sn: SnapshotFile, opts: &Opts) -> Option<SnapshotFile> {
    let mut changed = false;

    if !opts.set.is_empty() {
        changed |= sn.set_tags(opts.set.clone());
    }
    changed |= sn.add_tags(opts.add.clone());
    changed |= sn.remove_tags(opts.remove.clone());

    changed.then(|| sn)
}
