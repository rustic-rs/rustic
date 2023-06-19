//! `check` example
use rustic_core::{CheckOpts, NoProgressBars, Repository, RepositoryOptions};
use simplelog::{Config, LevelFilter, SimpleLogger};

fn main() {
    // Display info logs
    let _ = SimpleLogger::init(LevelFilter::Info, Config::default());

    // Open repository
    let mut repo_opts = RepositoryOptions::default();
    repo_opts.repository = Some("/tmp/repo".to_string());
    repo_opts.password = Some("test".to_string());
    let repo = Repository::new(&repo_opts).unwrap().open().unwrap();

    // Check respository with standard options
    let opts = CheckOpts::default();
    let progress = NoProgressBars {};
    repo.check(opts, &progress).unwrap()
}
