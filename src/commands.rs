//! Rustic Subcommands

pub(crate) mod backup;
pub(crate) mod cat;
pub(crate) mod check;
pub(crate) mod completions;
pub(crate) mod config;
pub(crate) mod copy;
pub(crate) mod diff;
pub(crate) mod dump;
pub(crate) mod forget;
pub(crate) mod init;
pub(crate) mod key;
pub(crate) mod list;
pub(crate) mod ls;
pub(crate) mod merge;
pub(crate) mod prune;
pub(crate) mod repair;
pub(crate) mod repoinfo;
pub(crate) mod restore;
pub(crate) mod self_update;
pub(crate) mod show_config;
pub(crate) mod snapshots;
pub(crate) mod tag;
#[cfg(feature = "webdav")]
pub(crate) mod webdav;

use std::fs::File;
use std::path::PathBuf;
use std::str::FromStr;

#[cfg(feature = "webdav")]
use crate::commands::webdav::WebDavCmd;
use crate::{
    commands::{
        backup::BackupCmd, cat::CatCmd, check::CheckCmd, completions::CompletionsCmd,
        config::ConfigCmd, copy::CopyCmd, diff::DiffCmd, dump::DumpCmd, forget::ForgetCmd,
        init::InitCmd, key::KeyCmd, list::ListCmd, ls::LsCmd, merge::MergeCmd, prune::PruneCmd,
        repair::RepairCmd, repoinfo::RepoInfoCmd, restore::RestoreCmd, self_update::SelfUpdateCmd,
        show_config::ShowConfigCmd, snapshots::SnapshotCmd, tag::TagCmd,
    },
    config::{progress_options::ProgressOptions, AllRepositoryOptions, RusticConfig},
    {Application, RUSTIC_APP},
};

use abscissa_core::{
    config::Override, terminal::ColorChoice, Command, Configurable, FrameworkError,
    FrameworkErrorKind, Runnable, Shutdown,
};
use anyhow::{anyhow, Result};
use clap::builder::{
    styling::{AnsiColor, Effects},
    Styles,
};
use dialoguer::Password;
use log::{log, Level};
use rustic_core::{OpenStatus, Repository};
use simplelog::{CombinedLogger, LevelFilter, TermLogger, TerminalMode, WriteLogger};

pub(super) mod constants {
    pub(super) const MAX_PASSWORD_RETRIES: usize = 5;
}

/// Rustic Subcommands
/// Subcommands need to be listed in an enum.
#[derive(clap::Parser, Command, Debug, Runnable)]
enum RusticCmd {
    /// Backup to the repository
    Backup(BackupCmd),

    /// Show raw data of repository files and blobs
    Cat(CatCmd),

    /// Change the repository configuration
    Config(ConfigCmd),

    /// Generate shell completions
    Completions(CompletionsCmd),

    /// Check the repository
    Check(CheckCmd),

    /// Copy snapshots to other repositories. Note: The target repositories must be given in the config file!
    Copy(CopyCmd),

    /// Compare two snapshots/paths
    /// Note that the exclude options only apply for comparison with a local path
    Diff(DiffCmd),

    /// dump the contents of a file in a snapshot to stdout
    Dump(DumpCmd),

    /// Remove snapshots from the repository
    Forget(ForgetCmd),

    /// Initialize a new repository
    Init(InitCmd),

    /// Manage keys
    Key(KeyCmd),

    /// List repository files
    List(ListCmd),

    /// List file contents of a snapshot
    Ls(LsCmd),

    /// Merge snapshots
    Merge(MergeCmd),

    /// Show a detailed overview of the snapshots within the repository
    Snapshots(SnapshotCmd),

    /// Show the configuration which has been read from the config file(s)
    ShowConfig(ShowConfigCmd),

    /// Update to the latest rustic release
    #[cfg_attr(not(feature = "self-update"), clap(hide = true))]
    SelfUpdate(SelfUpdateCmd),

    /// Remove unused data or repack repository pack files
    Prune(PruneCmd),

    /// Restore a snapshot/path
    Restore(RestoreCmd),

    /// Repair a snapshot/path
    Repair(RepairCmd),

    /// Show general information about the repository
    Repoinfo(RepoInfoCmd),

    /// Change tags of snapshots
    Tag(TagCmd),

    /// Start a webdav server which allows to access the repository
    #[cfg(feature = "webdav")]
    Webdav(WebDavCmd),
}

fn styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Red.on_default() | Effects::BOLD)
        .usage(AnsiColor::Red.on_default() | Effects::BOLD)
        .literal(AnsiColor::Blue.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::Green.on_default())
}

/// Entry point for the application. It needs to be a struct to allow using subcommands!
#[derive(clap::Parser, Command, Debug)]
#[command(author, about, name="rustic", styles=styles(), version = option_env!("PROJECT_VERSION").unwrap_or(env!("CARGO_PKG_VERSION")))]
pub struct EntryPoint {
    #[command(flatten)]
    pub config: RusticConfig,

    #[command(subcommand)]
    commands: RusticCmd,
}

impl Runnable for EntryPoint {
    fn run(&self) {
        self.commands.run();
        RUSTIC_APP.shutdown(Shutdown::Graceful)
    }
}

/// This trait allows you to define how application configuration is loaded.
impl Configurable<RusticConfig> for EntryPoint {
    /// Location of the configuration file
    fn config_path(&self) -> Option<PathBuf> {
        // Actually abscissa itself reads a config from `config_path`, but I have now returned None,
        // i.e. no config is read.
        None
    }

    /// Apply changes to the config after it's been loaded, e.g. overriding
    /// values in a config file using command-line options.
    fn process_config(&self, _config: RusticConfig) -> Result<RusticConfig, FrameworkError> {
        // Note: The config that is "not read" is then read here in `process_config()` by the
        // rustic logic and merged with the CLI options.
        // That's why it says `_config`, because it's not read at all and therefore not needed.
        let mut config = self.config.clone();

        // collect logs during merging as we start the logger *after* merging
        let mut merge_logs = Vec::new();

        // get global options from command line / env and config file
        if config.global.use_profile.is_empty() {
            config.merge_profile("rustic", &mut merge_logs, Level::Info)?;
        } else {
            for profile in &config.global.use_profile.clone() {
                config.merge_profile(profile, &mut merge_logs, Level::Warn)?;
            }
        }

        // start logger
        let level_filter = match &config.global.log_level {
            Some(level) => LevelFilter::from_str(level)
                .map_err(|e| FrameworkErrorKind::ConfigError.context(e))?,
            None => LevelFilter::Info,
        };
        match &config.global.log_file {
            None => TermLogger::init(
                level_filter,
                simplelog::ConfigBuilder::new()
                    .set_time_level(LevelFilter::Off)
                    .build(),
                TerminalMode::Stderr,
                ColorChoice::Auto,
            )
            .map_err(|e| FrameworkErrorKind::ConfigError.context(e))?,

            Some(file) => CombinedLogger::init(vec![
                TermLogger::new(
                    level_filter.min(LevelFilter::Warn),
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
            ])
            .map_err(|e| FrameworkErrorKind::ConfigError.context(e))?,
        }

        // display logs from merging
        for (level, merge_log) in merge_logs {
            log!(level, "{}", merge_log);
        }

        match &self.commands {
            RusticCmd::Forget(cmd) => cmd.override_config(config),
            #[cfg(feature = "webdav")]
            RusticCmd::Webdav(cmd) => cmd.override_config(config),

            // subcommands that don't need special overrides use a catch all
            _ => Ok(config),
        }
    }
}

/// Get the repository with the given options
///
/// # Arguments
///
/// * `repo_opts` - The repository options
///
fn get_repository(repo_opts: &AllRepositoryOptions) -> Result<Repository<ProgressOptions, ()>> {
    let po = RUSTIC_APP.config().global.progress_options;
    let backends = repo_opts.be.to_backends()?;
    let repo = Repository::new_with_progress(&repo_opts.repo, backends, po)?;
    Ok(repo)
}

/// Open the repository with the given options
///
/// # Arguments
///
/// * `repo_opts` - The repository options
///
/// # Errors
///
/// * [`RepositoryErrorKind::ReadingPasswordFromReaderFailed`] - If reading the password failed
/// * [`RepositoryErrorKind::OpeningPasswordFileFailed`] - If opening the password file failed
/// * [`RepositoryErrorKind::PasswordCommandParsingFailed`] - If parsing the password command failed
/// * [`RepositoryErrorKind::ReadingPasswordFromCommandFailed`] - If reading the password from the command failed
/// * [`RepositoryErrorKind::FromSplitError`] - If splitting the password command failed
///
/// [`RepositoryErrorKind::ReadingPasswordFromReaderFailed`]: crate::error::RepositoryErrorKind::ReadingPasswordFromReaderFailed
/// [`RepositoryErrorKind::OpeningPasswordFileFailed`]: crate::error::RepositoryErrorKind::OpeningPasswordFileFailed
/// [`RepositoryErrorKind::PasswordCommandParsingFailed`]: crate::error::RepositoryErrorKind::PasswordCommandParsingFailed
/// [`RepositoryErrorKind::ReadingPasswordFromCommandFailed`]: crate::error::RepositoryErrorKind::ReadingPasswordFromCommandFailed
/// [`RepositoryErrorKind::FromSplitError`]: crate::error::RepositoryErrorKind::FromSplitError
fn open_repository(
    repo_opts: &AllRepositoryOptions,
) -> Result<Repository<ProgressOptions, OpenStatus>> {
    let repo = get_repository(repo_opts)?;
    match repo.password()? {
        // if password is given, directly return the result of find_key_in_backend and don't retry
        Some(pass) => {
            return Ok(repo.open_with_password(&pass)?);
        }
        None => {
            for _ in 0..constants::MAX_PASSWORD_RETRIES {
                let pass = Password::new()
                    .with_prompt("enter repository password")
                    .allow_empty_password(true)
                    .interact()?;
                match repo.clone().open_with_password(&pass) {
                    Ok(repo) => return Ok(repo),
                    Err(err) if err.is_incorrect_password() => continue,
                    Err(err) => return Err(err.into()),
                }
            }
        }
    }
    Err(anyhow!("incorrect password"))
}

#[cfg(test)]
mod tests {
    use crate::commands::EntryPoint;
    use clap::CommandFactory;

    #[test]
    fn verify_cli() {
        EntryPoint::command().debug_assert();
    }
}
