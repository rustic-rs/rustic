//! Rustic Subcommands

pub(crate) mod backup;
pub(crate) mod cat;
pub(crate) mod check;
pub(crate) mod completions;
pub(crate) mod config;
pub(crate) mod copy;
pub(crate) mod diff;
pub(crate) mod docs;
pub(crate) mod dump;
pub(crate) mod find;
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
#[cfg(feature = "tui")]
pub(crate) mod tui;
#[cfg(feature = "webdav")]
pub(crate) mod webdav;

use std::fmt::Debug;
use std::fs::File;
use std::path::PathBuf;
use std::str::FromStr;

#[cfg(feature = "webdav")]
use crate::commands::webdav::WebDavCmd;
use crate::{
    commands::{
        backup::BackupCmd, cat::CatCmd, check::CheckCmd, completions::CompletionsCmd,
        config::ConfigCmd, copy::CopyCmd, diff::DiffCmd, docs::DocsCmd, dump::DumpCmd,
        forget::ForgetCmd, init::InitCmd, key::KeyCmd, list::ListCmd, ls::LsCmd, merge::MergeCmd,
        prune::PruneCmd, repair::RepairCmd, repoinfo::RepoInfoCmd, restore::RestoreCmd,
        self_update::SelfUpdateCmd, show_config::ShowConfigCmd, snapshots::SnapshotCmd,
        tag::TagCmd,
    },
    config::RusticConfig,
    Application, RUSTIC_APP,
};

use abscissa_core::{
    config::Override, terminal::ColorChoice, Command, Configurable, FrameworkError,
    FrameworkErrorKind, Runnable, Shutdown,
};
use anyhow::Result;
use clap::builder::{
    styling::{AnsiColor, Effects},
    Styles,
};
use convert_case::{Case, Casing};
use human_panic::setup_panic;
use log::{log, Level};
use simplelog::{CombinedLogger, LevelFilter, TermLogger, TerminalMode, WriteLogger};

use self::find::FindCmd;

/// Rustic Subcommands
/// Subcommands need to be listed in an enum.
#[derive(clap::Parser, Command, Debug, Runnable)]
enum RusticCmd {
    /// Backup to the repository
    Backup(BackupCmd),

    /// Show raw data of files and blobs in a repository
    Cat(CatCmd),

    /// Change the repository configuration
    Config(ConfigCmd),

    /// Generate shell completions
    Completions(CompletionsCmd),

    /// Check the repository
    Check(CheckCmd),

    /// Copy snapshots to other repositories
    Copy(CopyCmd),

    /// Compare two snapshots or paths
    Diff(DiffCmd),

    /// Open the documentation
    Docs(DocsCmd),

    /// Dump the contents of a file within a snapshot to stdout
    Dump(DumpCmd),

    /// Find patterns in given snapshots
    Find(FindCmd),

    /// Remove snapshots from the repository
    Forget(ForgetCmd),

    /// Initialize a new repository
    Init(InitCmd),

    /// Manage keys for a repository
    Key(KeyCmd),

    /// List repository files by file type
    List(ListCmd),

    /// List file contents of a snapshot
    Ls(LsCmd),

    /// Merge snapshots
    Merge(MergeCmd),

    /// Show a detailed overview of the snapshots within the repository
    Snapshots(SnapshotCmd),

    /// Show the configuration which has been read from the config file(s)
    ShowConfig(ShowConfigCmd),

    /// Update to the latest stable rustic release
    #[cfg_attr(not(feature = "self-update"), clap(hide = true))]
    SelfUpdate(SelfUpdateCmd),

    /// Remove unused data or repack repository pack files
    Prune(PruneCmd),

    /// Restore (a path within) a snapshot
    Restore(RestoreCmd),

    /// Repair a snapshot or the repository index
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
        // Set up panic hook for better error messages and logs
        setup_panic!();

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

        // collect "RUSTIC_REPO_OPT*" and "OPENDAL_*" env variables
        for (var, value) in std::env::vars() {
            if let Some(var) = var.strip_prefix("RUSTIC_REPO_OPT_") {
                let var = var.from_case(Case::UpperSnake).to_case(Case::Kebab);
                _ = config.repository.be.options.insert(var, value);
            } else if let Some(var) = var.strip_prefix("OPENDAL_") {
                let var = var.from_case(Case::UpperSnake).to_case(Case::Snake);
                _ = config.repository.be.options.insert(var, value);
            } else if let Some(var) = var.strip_prefix("RUSTIC_REPO_OPTHOT_") {
                let var = var.from_case(Case::UpperSnake).to_case(Case::Kebab);
                _ = config.repository.be.options_hot.insert(var, value);
            } else if let Some(var) = var.strip_prefix("RUSTIC_REPO_OPTCOLD_") {
                let var = var.from_case(Case::UpperSnake).to_case(Case::Kebab);
                _ = config.repository.be.options_cold.insert(var, value);
            }
        }

        // collect logs during merging as we start the logger *after* merging
        let mut merge_logs = Vec::new();

        // get global options from command line / env and config file
        if config.global.use_profiles.is_empty() {
            config.merge_profile("rustic", &mut merge_logs, Level::Info)?;
        } else {
            for profile in &config.global.use_profiles.clone() {
                config.merge_profile(profile, &mut merge_logs, Level::Warn)?;
            }
        }

        // start logger
        let level_filter = match &config.global.log_level {
            Some(level) => LevelFilter::from_str(level)
                .map_err(|e| FrameworkErrorKind::ConfigError.context(e))?,
            None => LevelFilter::Info,
        };
        let term_config = simplelog::ConfigBuilder::new()
            .set_time_level(LevelFilter::Off)
            .build();
        match &config.global.log_file {
            None => TermLogger::init(
                level_filter,
                term_config,
                TerminalMode::Stderr,
                ColorChoice::Auto,
            )
            .map_err(|e| FrameworkErrorKind::ConfigError.context(e))?,

            Some(file) => {
                let file_config = simplelog::ConfigBuilder::new()
                    .set_time_format_rfc3339()
                    .build();
                let file = File::options()
                    .create(true)
                    .append(true)
                    .open(file)
                    .map_err(|e| {
                        FrameworkErrorKind::PathError {
                            name: Some(file.clone()),
                        }
                        .context(e)
                    })?;
                let term_logger = TermLogger::new(
                    level_filter.min(LevelFilter::Warn),
                    term_config,
                    TerminalMode::Stderr,
                    ColorChoice::Auto,
                );
                CombinedLogger::init(vec![
                    term_logger,
                    WriteLogger::new(level_filter, file_config, file),
                ])
                .map_err(|e| FrameworkErrorKind::ConfigError.context(e))?;
            }
        }

        // display logs from merging
        for (level, merge_log) in merge_logs {
            log!(level, "{}", merge_log);
        }

        match &self.commands {
            RusticCmd::Forget(cmd) => cmd.override_config(config),
            RusticCmd::Copy(cmd) => cmd.override_config(config),
            #[cfg(feature = "webdav")]
            RusticCmd::Webdav(cmd) => cmd.override_config(config),

            // subcommands that don't need special overrides use a catch all
            _ => Ok(config),
        }
    }
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
