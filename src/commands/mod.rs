use std::fs::File;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use merge::Merge;
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};
use simplelog::*;

use crate::backend::{FileType, ReadBackend};
use crate::repository::{Repository, RepositoryOptions};

use helpers::*;

mod backup;
mod cat;
mod check;
mod completions;
mod config;
mod diff;
mod forget;
mod helpers;
mod init;
mod key;
mod list;
mod ls;
mod prune;
mod repair;
mod repoinfo;
mod restore;
mod rustic_config;
mod self_update;
mod snapshots;
mod tag;

use rustic_config::RusticConfig;

#[derive(Parser)]
#[clap(about, name="rustic", version = option_env!("PROJECT_VERSION").unwrap_or(env!("CARGO_PKG_VERSION")))]
struct Opts {
    #[clap(flatten, help_heading = "GLOBAL OPTIONS")]
    global: GlobalOpts,

    #[clap(flatten, help_heading = "REPOSITORY OPTIONS")]
    repository: RepositoryOptions,

    /// Config profile to use. This parses the file <PROFILE>.toml in the config directory.
    #[clap(
        short = 'P',
        long,
        value_name = "PROFILE",
        global = true,
        default_value = "rustic"
    )]
    config_profile: String,

    #[clap(subcommand)]
    command: Command,
}

#[serde_as]
#[derive(Default, Parser, Deserialize, Merge)]
#[serde(default, rename_all = "kebab-case")]
struct GlobalOpts {
    /// Use this log level [default: info]
    #[clap(long, global = true, env = "RUSTIC_LOG_LEVEL")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    log_level: Option<LevelFilter>,

    /// Write log messages to the given file instead of printing them.
    /// Note: warnings and errors are still additionally printed unless they are ignored by --log-level
    #[clap(long, global = true, env = "RUSTIC_LOG_FILE", value_name = "LOGFILE")]
    log_file: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Backup to the repository
    Backup(backup::Opts),

    /// Show raw data of repository files and blobs
    Cat(cat::Opts),

    /// Change the repository configuration
    Config(config::Opts),

    /// Generate shell completions
    Completions(completions::Opts),

    /// Check the repository
    Check(check::Opts),

    /// Compare two snapshots/paths
    ///
    /// Note that the exclude options only apply for comparison with a local path
    Diff(diff::Opts),

    /// Remove snapshots from the repository
    Forget(forget::Opts),

    /// Initialize a new repository
    Init(init::Opts),

    /// Manage keys
    Key(key::Opts),

    /// List repository files
    List(list::Opts),

    /// List file contents of a snapshot
    Ls(ls::Opts),

    /// Show a detailed overview of the snapshots within the repository
    Snapshots(snapshots::Opts),

    /// Update to the latest rustic release
    SelfUpdate(self_update::Opts),

    /// Remove unused data or repack repository pack files
    Prune(prune::Opts),

    /// Restore a snapshot/path
    Restore(restore::Opts),

    /// Restore a snapshot/path
    Repair(repair::Opts),

    /// Show general information about the repository
    Repoinfo(repoinfo::Opts),

    /// Change tags of snapshots
    Tag(tag::Opts),
}

pub fn execute() -> Result<()> {
    let command: Vec<_> = std::env::args_os().into_iter().collect();
    let args = Opts::parse_from(&command);

    // get global options from command line / env and config file
    let config_file = RusticConfig::new(&args.config_profile)?;
    let mut opts = args.global;
    config_file.merge_into("global", &mut opts)?;

    // start logger
    let level_filter = opts.log_level.unwrap_or(LevelFilter::Info);
    match opts.log_file {
        None => TermLogger::init(
            level_filter,
            ConfigBuilder::new()
                .set_time_level(LevelFilter::Off)
                .build(),
            TerminalMode::Stderr,
            ColorChoice::Auto,
        )?,

        Some(file) => CombinedLogger::init(vec![
            TermLogger::new(
                level_filter.max(LevelFilter::Warn),
                ConfigBuilder::new()
                    .set_time_level(LevelFilter::Off)
                    .build(),
                TerminalMode::Stderr,
                ColorChoice::Auto,
            ),
            WriteLogger::new(
                level_filter,
                Config::default(),
                File::options().create(true).append(true).open(file)?,
            ),
        ])?,
    }

    if let Command::SelfUpdate(opts) = args.command {
        self_update::execute(opts)?;
        return Ok(());
    }

    if let Command::Completions(opts) = args.command {
        completions::execute(opts);
        return Ok(());
    }

    let command: String = command
        .into_iter()
        .map(|s| s.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(" ");

    let mut repo_opts = args.repository;
    config_file.merge_into("repository", &mut repo_opts)?;
    config_file.merge_into("global", &mut repo_opts)?; // deprecated, but repo-options were originally under [global]
    let repo = Repository::new(repo_opts)?;

    if let Command::Init(opts) = args.command {
        let config_ids = repo.be.list(FileType::Config)?;
        return init::execute(&repo.be, &repo.be_hot, opts, repo.password()?, config_ids);
    }

    let repo = repo.open()?;

    match args.command {
        Command::Backup(opts) => backup::execute(repo, opts, config_file, command)?,
        Command::Config(opts) => config::execute(repo, opts)?,
        Command::Cat(opts) => cat::execute(repo, opts, config_file)?,
        Command::Check(opts) => check::execute(repo, opts)?,
        Command::Completions(_) => {} // already handled above
        Command::Diff(opts) => diff::execute(repo, opts, config_file)?,
        Command::Forget(opts) => forget::execute(repo, opts, config_file)?,
        Command::Init(_) => {} // already handled above
        Command::Key(opts) => key::execute(repo, opts)?,
        Command::List(opts) => list::execute(repo, opts)?,
        Command::Ls(opts) => ls::execute(repo, opts, config_file)?,
        Command::SelfUpdate(_) => {} // already handled above
        Command::Snapshots(opts) => snapshots::execute(repo, opts, config_file)?,
        Command::Prune(opts) => prune::execute(repo, opts, vec![])?,
        Command::Restore(opts) => restore::execute(repo, opts, config_file)?,
        Command::Repair(opts) => repair::execute(repo, opts, config_file)?,
        Command::Repoinfo(opts) => repoinfo::execute(repo, opts)?,
        Command::Tag(opts) => tag::execute(repo, opts, config_file)?,
    };

    Ok(())
}
