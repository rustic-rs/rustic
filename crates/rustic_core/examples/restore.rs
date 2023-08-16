//! `restore` example
use rustic_core::{LocalDestination, LsOptions, Repository, RepositoryOptions, RestoreOptions};
use simplelog::{Config, LevelFilter, SimpleLogger};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // Display info logs
    let _ = SimpleLogger::init(LevelFilter::Info, Config::default());

    // Open repository
    let repo_opts = RepositoryOptions::default()
        .repository("/tmp/repo")
        .password("test");
    let repo = Repository::new(&repo_opts)?.open()?.to_indexed()?;

    // use latest snapshot without filtering snapshots
    let node = repo.node_from_snapshot_path("latest", |_| true)?;

    // use list of the snapshot contents using no additional filtering
    let streamer_opts = LsOptions::default();
    let ls = repo.ls(&node, &streamer_opts)?;

    let destination = "./restore/"; // restore to this destination dir
    let create = true; // create destination dir, if it doesn't exist
    let dest = LocalDestination::new(destination, create, !node.is_dir())?;

    let opts = RestoreOptions::default();
    let dry_run = false;
    // create restore infos. Note: this also already creates needed dirs in the destination
    let restore_infos = repo.prepare_restore(&opts, ls.clone(), &dest, dry_run)?;

    repo.restore(restore_infos, &opts, ls, &dest)?;
    Ok(())
}
