use std::fs::File;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use merge::Merge;
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};
use simplelog::{ColorChoice, CombinedLogger, LevelFilter, TermLogger, TerminalMode, WriteLogger};

use crate::backend::{FileType, ReadBackend};
use crate::repository::Repository;

use helpers::*;

mod backup;
mod cat;
mod check;
mod completions;
mod config;
mod configfile;
mod copy;
mod diff;
mod dump;
mod forget;
mod helpers;
mod init;
mod key;
mod list;
mod ls;
mod merge_cmd;
mod prune;
mod repair;
mod repoinfo;
mod restore;
mod self_update;
mod snapshots;
mod tag;

use configfile::Config;

#[derive(Parser)]
#[clap(about, version, name="rustic", version = option_env!("PROJECT_VERSION").unwrap_or(env!("CARGO_PKG_VERSION")))]
struct Args {
    #[clap(flatten)]
    config: Config,

    #[clap(subcommand)]
    command: Command,
}

#[serde_as]
#[derive(Default, Debug, Parser, Deserialize, Merge)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct GlobalOpts {
    /// Config profile to use. This parses the file `<PROFILE>.toml` in the config directory.
    /// [default: "rustic"]
    #[clap(
        short = 'P',
        long,
        global = true,
        value_name = "PROFILE",
        env = "RUSTIC_USE_PROFILE"
    )]
    #[merge(strategy = merge::vec::append)]
    use_profile: Vec<String>,

    /// Only show what would be done without modifying anything. Does not affect read-only commands
    #[clap(long, short = 'n', global = true, env = "RUSTIC_DRY_RUN")]
    #[merge(strategy = merge::bool::overwrite_false)]
    dry_run: bool,

    /// Use this log level [default: info]
    #[clap(long, global = true, env = "RUSTIC_LOG_LEVEL")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    log_level: Option<LevelFilter>,

    /// Write log messages to the given file instead of printing them.
    /// Note: warnings and errors are still additionally printed unless they are ignored by --log-level
    #[clap(long, global = true, env = "RUSTIC_LOG_FILE", value_name = "LOGFILE")]
    log_file: Option<PathBuf>,

    /// Don't show any progress bar
    #[clap(long, global = true, env = "RUSTIC_NO_PROGRESS")]
    #[merge(strategy=merge::bool::overwrite_false)]
    no_progress: bool,

    /// Interval to update progress bars
    #[clap(
        long,
        global = true,
        env = "RUSTIC_PROGRESS_INTERVAL",
        value_name = "DURATION",
        conflicts_with = "no_progress"
    )]
    #[serde_as(as = "Option<DisplayFromStr>")]
    progress_interval: Option<humantime::Duration>,
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

    /// Copy snapshots to other repositories. Note: The target repositories must be given in the config file!
    Copy(copy::Opts),

    /// Compare two snapshots/paths
    /// Note that the exclude options only apply for comparison with a local path
    Diff(diff::Opts),

    /// dump the contents of a file in a snapshot to stdout
    Dump(dump::Opts),

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

    /// Merge snapshots
    Merge(merge_cmd::Opts),

    /// Show a detailed overview of the snapshots within the repository
    Snapshots(snapshots::Opts),

    /// Show the configuration which has been read from the config file(s)
    ShowConfig,

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
    let command: Vec<_> = std::env::args_os().collect();
    let args = Args::parse_from(&command);
    let mut config = args.config;
    if config.global.use_profile.is_empty() {
        config.global.use_profile.push("rustic".to_string());
    }

    // get global options from command line / env and config file
    for profile in &config.global.use_profile.clone() {
        config.merge_profile(profile)?;
    }

    if let Command::ShowConfig = args.command {
        println!("{config:#?}");
        return Ok(());
    }

    // start logger
    let level_filter = config.global.log_level.unwrap_or(LevelFilter::Info);
    match &config.global.log_file {
        None => TermLogger::init(
            level_filter,
            simplelog::ConfigBuilder::new()
                .set_time_level(LevelFilter::Off)
                .build(),
            TerminalMode::Stderr,
            ColorChoice::Auto,
        )?,

        Some(file) => CombinedLogger::init(vec![
            TermLogger::new(
                level_filter.max(LevelFilter::Warn),
                simplelog::ConfigBuilder::new()
                    .set_time_level(LevelFilter::Off)
                    .build(),
                TerminalMode::Stderr,
                ColorChoice::Auto,
            ),
            WriteLogger::new(
                level_filter,
                simplelog::Config::default(),
                File::options().create(true).append(true).open(file)?,
            ),
        ])?,
    }

    if config.global.no_progress {
        let mut no_progress = NO_PROGRESS.lock().unwrap();
        *no_progress = true;
    }

    if let Some(duration) = config.global.progress_interval {
        let mut interval = PROGRESS_INTERVAL.lock().unwrap();
        *interval = *duration;
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

    let repo = Repository::new(config.repository.clone())?;

    if let Command::Init(opts) = args.command {
        let config_ids = repo.be.list(FileType::Config)?;
        return init::execute(&repo.be, &repo.be_hot, opts, repo.password()?, config_ids);
    }

    let repo = repo.open()?;

    #[allow(clippy::match_same_arms)]
    match args.command {
        Command::Backup(opts) => backup::execute(repo, config, opts, command)?,
        Command::Config(opts) => config::execute(repo, opts)?,
        Command::Cat(opts) => cat::execute(repo, config, opts)?,
        Command::Check(opts) => check::execute(repo, opts)?,
        Command::Completions(_) => {} // already handled above
        Command::Copy(opts) => copy::execute(repo, config, opts)?,
        Command::Diff(opts) => diff::execute(repo, config, opts)?,
        Command::Dump(opts) => dump::execute(repo, config, opts)?,
        Command::Forget(opts) => forget::execute(repo, config, opts)?,
        Command::Init(_) => {} // already handled above
        Command::Key(opts) => key::execute(repo, opts)?,
        Command::List(opts) => list::execute(repo, opts)?,
        Command::Ls(opts) => ls::execute(repo, config, opts)?,
        Command::Merge(opts) => merge_cmd::execute(repo, config, opts, command)?,
        Command::SelfUpdate(_) => {} // already handled above
        Command::Snapshots(opts) => snapshots::execute(repo, config, opts)?,
        Command::ShowConfig => {} // already handled above
        Command::Prune(opts) => prune::execute(repo, config, opts, vec![])?,
        Command::Restore(opts) => restore::execute(repo, config, opts)?,
        Command::Repair(opts) => repair::execute(repo, config, opts)?,
        Command::Repoinfo(opts) => repoinfo::execute(repo, opts)?,
        Command::Tag(opts) => tag::execute(repo, config, opts)?,
    };

    Ok(())
}

#[test]
fn verify_cli() {
    use clap::CommandFactory;
    Args::command().debug_assert()
}
