//! `key` example
use rustic_core::{KeyOpts, Repository, RepositoryOptions, RusticPassword};
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

    // Add a new key with the given password
    let key_opts = KeyOpts::default();
    let pass = RusticPassword::new("new_password").into();
    repo.add_key(pass, &key_opts)?;
    Ok(())
}
