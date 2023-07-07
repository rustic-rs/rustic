//! `key` example
use rustic_core::{KeyOpts, Repository, RepositoryOptions};
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

    let repo = Repository::new(&repo_opts).unwrap().open().unwrap();

    // Add a new key with the given password
    let key_opts = KeyOpts::default();
    repo.add_key("new_password", &key_opts).unwrap();
}
