//! `init` example
use rustic_core::{ConfigOptions, KeyOptions, Repository, RepositoryOptions};
use simplelog::{Config, LevelFilter, SimpleLogger};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // Display info logs
    let _ = SimpleLogger::init(LevelFilter::Info, Config::default());

    // Init repository
    let repo_opts = RepositoryOptions::default()
        .repository("/tmp/repo")
        .password("test");
    let key_opts = KeyOptions::default();
    let config_opts = ConfigOptions::default();
    let _repo = Repository::new(&repo_opts)?.init(&key_opts, &config_opts)?;

    // -> use _repo for any operation on an open repository
    Ok(())
}
