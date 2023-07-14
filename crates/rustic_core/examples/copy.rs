//! `copy` example
use rustic_core::{CopySnapshot, Repository, RepositoryOptions};
use simplelog::{Config, LevelFilter, SimpleLogger};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // Display info logs
    let _ = SimpleLogger::init(LevelFilter::Info, Config::default());

    // Open repository
    let src_repo_opts = RepositoryOptions::default()
        .repository("/tmp/repo")
        .password("test");
    let src_repo = Repository::new(&src_repo_opts)?.open()?.to_indexed()?;

    let dst_repo_opts = RepositoryOptions::default()
        .repository("tmp/repo")
        .password("test");
    let dst_repo = Repository::new(&dst_repo_opts)?.open()?.to_indexed_ids()?;

    // get snapshots which are missing in dst_repo
    let snapshots = src_repo.get_all_snapshots()?;
    let snaps = dst_repo.relevant_copy_snapshots(|_| true, &snapshots)?;

    // copy only relevant snapshots
    src_repo.copy(
        &dst_repo,
        snaps
            .iter()
            .filter_map(|CopySnapshot { relevant, sn }| relevant.then_some(sn)),
    )?;
    Ok(())
}
