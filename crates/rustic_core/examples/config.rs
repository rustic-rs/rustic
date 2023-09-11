//! `config` example
use rustic_core::{max_compression_level, ConfigOptions, Repository, RepositoryOptions};
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

    // Set Config, e.g. Compression level
    let config_opts = ConfigOptions::default().set_compression(max_compression_level());
    repo.apply_config(&config_opts)?;
    Ok(())
}
