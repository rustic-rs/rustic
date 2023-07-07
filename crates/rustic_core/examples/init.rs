//! `init` example
use rustic_core::{ConfigOpts, KeyOpts, Repository, RepositoryOptions};
use simplelog::{Config, LevelFilter, SimpleLogger};

fn main() {
    // Display info logs
    let _ = SimpleLogger::init(LevelFilter::Info, Config::default());

    // Init repository
    let repo_opts = RepositoryOptions {
        repository: Some("/tmp/repo".to_string()),
        password: Some("test".to_string()),
        ..Default::default()
    };
    let key_opts = KeyOpts::default();
    let config_opts = ConfigOpts::default();
    let _repo = Repository::new(&repo_opts)
        .unwrap()
        .init(&key_opts, &config_opts)
        .unwrap();

    // -> use _repo for any operation on an open repository
}
