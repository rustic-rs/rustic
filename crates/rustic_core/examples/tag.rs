//! `tag` example
use std::str::FromStr;

use rustic_core::{Repository, RepositoryOptions, StringList};
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

    // Get all snapshots - use a filter which doesn't filter out.
    let snaps = repo.get_matching_snapshots(|_| true).unwrap();

    // Set tag "test" to all snapshots, filtering out unchanged (i.e. tag was aready preset) snapshots
    let tags = vec![StringList::from_str("test").unwrap()];
    let snaps: Vec<_> = snaps
        .into_iter()
        .filter_map(|mut sn| sn.add_tags(tags.clone()).then_some(sn)) // can also use set_tags or remove_tags
        .collect();
    let old_snap_ids: Vec<_> = snaps.iter().map(|sn| sn.id).collect();

    // remove old snapshots and save changed ones
    repo.save_snapshots(snaps).unwrap();
    repo.delete_snapshots(&old_snap_ids).unwrap();
}
