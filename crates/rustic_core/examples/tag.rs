//! `tag` example
use rustic_core::{Repository, RepositoryOptions, StringList};
use simplelog::{Config, LevelFilter, SimpleLogger};
use std::error::Error;
use std::str::FromStr;

fn main() -> Result<(), Box<dyn Error>> {
    // Display info logs
    let _ = SimpleLogger::init(LevelFilter::Info, Config::default());

    // Open repository
    let repo_opts = RepositoryOptions::default()
        .repository("/tmp/repo")
        .password("test");
    let repo = Repository::new(&repo_opts)?.open()?;

    // Set tag "test" to all snapshots, filtering out unchanged (i.e. tag was aready preset) snapshots
    let snaps = repo.get_all_snapshots()?;
    let tags = vec![StringList::from_str("test")?];
    let snaps: Vec<_> = snaps
        .into_iter()
        .filter_map(|mut sn| sn.add_tags(tags.clone()).then_some(sn)) // can also use set_tags or remove_tags
        .collect();
    let old_snap_ids: Vec<_> = snaps.iter().map(|sn| sn.id).collect();

    // remove old snapshots and save changed ones
    repo.save_snapshots(snaps)?;
    repo.delete_snapshots(&old_snap_ids)?;
    Ok(())
}
