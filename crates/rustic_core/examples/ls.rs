//! `ls` example
use rustic_core::{Repository, RepositoryOptions, TreeStreamerOptions};
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

    // recursively list the snapshot contents using no additional filtering
    let recursive = true;
    let streamer_opts = TreeStreamerOptions::default();
    for item in repo.ls(&node, &streamer_opts, recursive)? {
        let (path, _) = item?;
        println!("{path:?} ");
    }
    Ok(())
}
