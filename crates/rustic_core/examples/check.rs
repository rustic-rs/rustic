//! `check` example
use rustic_core::{CheckOptions, Repository, RepositoryOptions};
use simplelog::{Config, LevelFilter, SimpleLogger};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // Display info logs
    let _ = SimpleLogger::init(LevelFilter::Info, Config::default());

    // Open repository
    let repo_opts = RepositoryOptions::default()
        .repository("/tmp/repo")
        .password("test");
    let repo = Repository::new(&repo_opts)?.open()?;

    // Check respository with standard options but omitting cache checks
    let opts = CheckOptions::default().trust_cache(true);
    repo.check(opts)?;
    Ok(())
}
