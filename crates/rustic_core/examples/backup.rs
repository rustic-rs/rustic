//! `backup` example
use rustic_core::{BackupOpts, PathList, Repository, RepositoryOptions, SnapshotFile};
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

    let backup_opts = BackupOpts::default();
    let source = PathList::from_string(".", true)?; // true: sanitize the given string
    let dry_run = false;

    let snap = repo.backup(&backup_opts, source, SnapshotFile::default(), dry_run)?;

    println!("successfully created snapshot:\n{snap:#?}");
    Ok(())
}
