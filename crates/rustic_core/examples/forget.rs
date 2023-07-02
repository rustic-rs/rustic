//! `forget` example
use rustic_core::{KeepOptions, Repository, RepositoryOptions, SnapshotGroupCriterion};
use simplelog::{Config, LevelFilter, SimpleLogger};

fn main() {
    // Display info logs
    let _ = SimpleLogger::init(LevelFilter::Info, Config::default());

    // Open repository
    let repo_opts = RepositoryOptions {
        repository: Some("/tmp/repo".to_string()),
        password: Some("test".to_string()),
        ..Default::default()
    };
    let repo = Repository::new(&repo_opts).unwrap().open().unwrap();

    // Check respository with standard options
    let group_by = SnapshotGroupCriterion::default();
    let mut keep = KeepOptions::default();
    keep.keep_daily = 5;
    keep.keep_weekly = 10;
    let snaps = repo
        .get_forget_snapshots(&keep, group_by, |_| true)
        .unwrap();
    println!("{snaps:?}");
    // to remove the snapshots-to-forget, uncomment this line:
    // repo.delete_snapshots(&snaps.into_forget_ids()).unwrap()
}
