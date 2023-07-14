//! `backup` example
use rustic_core::{BackupOpts, PathList, Repository, RepositoryOptions, SnapshotFile};
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

    let repo = Repository::new(&repo_opts)
        .unwrap()
        .open()
        .unwrap()
        .to_indexed_ids()
        .unwrap();

    let backup_opts = BackupOpts::default();
    let source = PathList::from_string(".", true).unwrap(); // true: sanitize the given string
    let dry_run = false;

    let snap = repo
        .backup(&backup_opts, source, SnapshotFile::default(), dry_run)
        .unwrap();

    println!("successfully created snapshot:\n{snap:#?}")
}
