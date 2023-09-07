//! `forget` example
use rustic_core::{KeepOptions, Repository, RepositoryOptions, SnapshotGroupCriterion};
use simplelog::{Config, LevelFilter, SimpleLogger};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // Display info logs
    let _ = SimpleLogger::init(LevelFilter::Info, Config::default());

    // Open repository
    let repo_opts = RepositoryOptions::default()
        .repository("/tmp/repo")
        .password("test");
    let repo = Repository::new(&repo_opts)?.open()?;

    // Check respository with standard options
    let group_by = SnapshotGroupCriterion::default();
    let keep = KeepOptions::default().keep_daily(5).keep_weekly(10);
    let snaps = repo.get_forget_snapshots(&keep, group_by, |_| true)?;
    println!("{snaps:?}");
    // to remove the snapshots-to-forget, uncomment this line:
    // repo.delete_snapshots(&snaps.into_forget_ids())?
    Ok(())
}
