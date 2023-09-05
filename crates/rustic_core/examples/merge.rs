//! `merge` example
use rustic_core::{last_modified_node, repofile::SnapshotFile, Repository, RepositoryOptions};
use simplelog::{Config, LevelFilter, SimpleLogger};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // Display info logs
    let _ = SimpleLogger::init(LevelFilter::Info, Config::default());

    // Open repository
    let repo_opts = RepositoryOptions::default()
        .repository("/tmp/repo")
        .password("test");
    let repo = Repository::new(&repo_opts)?.open()?.to_indexed_ids()?;

    // Merge all snapshots using the latest entry for duplicate entries
    let snaps = repo.get_all_snapshots()?;
    // This creates a new snapshot without removing the used ones
    let snap = repo.merge_snapshots(&snaps, &last_modified_node, SnapshotFile::default())?;

    println!("successfully created snapshot:\n{snap:#?}");
    Ok(())
}
