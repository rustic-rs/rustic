use anyhow::Result;
use clap::Parser;

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
    // TODO: allow giving specific snapshots
}

pub(super) async fn execute(be: &impl DecryptFullBackend, opts: Opts) -> Result<()> {
    let snapshots = SnapshotFile::all_from_backend(be).await?;

    let mut count = 0;
    for sn in snapshots.into_iter().filter(|sn| sn.matches(&opts.filter)) {
        if modify_sn(sn, be, &opts).await? {
            count += 1;
        }
    }

    println!("changed {} snapshot(s)", count);

    Ok(())
}

async fn modify_sn(
    mut sn: SnapshotFile,
    be: &impl DecryptFullBackend,
    opts: &Opts,
) -> Result<bool> {
    let mut changed = false;

    if !opts.set.is_empty() {
        changed |= sn.set_tags(opts.set.clone());
    }
    changed |= sn.add_tags(opts.add.clone());
    changed |= sn.remove_tags(opts.remove.clone());

    // FIXME: For some reason, changed is always true...?!?
    if changed {
        // TODO: Save original snapshot ID
        // TODO: Save and delete in parallel
        be.save_file(&sn).await?;
        be.remove(FileType::Snapshot, &sn.id).await?;
    }

    Ok(changed)
}
