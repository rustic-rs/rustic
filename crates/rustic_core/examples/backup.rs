//! `backup` example
use rustic_core::{BackupOptions, PathList, Repository, RepositoryOptions, SnapshotOptions};
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

    let backup_opts = BackupOptions::default();
    let source = PathList::from_string(".")?.sanitize()?;
    let snap = SnapshotOptions::default()
        .add_tags("tag1,tag2")?
        .to_snapshot()?;

    // Create snapshot
    let snap = repo.backup(&backup_opts, source, snap)?;

    println!("successfully created snapshot:\n{snap:#?}");
    Ok(())
}
