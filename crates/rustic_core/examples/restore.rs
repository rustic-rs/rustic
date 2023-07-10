//! `restore` example
use rustic_core::{
    LocalDestination, Repository, RepositoryOptions, RestoreOpts, TreeStreamerOptions,
};
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
        .to_indexed()
        .unwrap();

    // use latest snapshot without filtering snapshots
    let node = repo.node_from_snapshot_path("latest", |_| true).unwrap();

    // use list of the snapshot contents using no additional filtering
    let recursive = true;
    let streamer_opts = TreeStreamerOptions::default();
    let ls = repo.ls(&node, &streamer_opts, recursive).unwrap();

    let destination = "./restore/"; // restore to this destination dir
    let create = true; // create destination dir, if it doesn't exist
    let dest = LocalDestination::new(destination, create, !node.is_dir()).unwrap();

    let opts = RestoreOpts::default();
    let dry_run = false;
    // create restore infos. Note: this also already creates needed dirs in the destination
    let restore_infos = repo
        .prepare_restore(&opts, ls.clone(), &dest, dry_run)
        .unwrap();

    repo.restore(restore_infos, &opts, ls, &dest).unwrap();
}
