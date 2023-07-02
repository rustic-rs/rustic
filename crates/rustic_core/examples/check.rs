//! `check` example
use rustic_core::{CheckOpts, Repository, RepositoryOptions};
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

    // Check respository with standard options
    let opts = CheckOpts::default();
    repo.check(opts).unwrap()
}
