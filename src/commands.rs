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

#[cfg(feature = "webdav")]
use crate::commands::webdav::WebDavCmd;
use crate::{
    application::RusticPaths,
    commands::{
        backup::BackupCmd, cat::CatCmd, check::CheckCmd, completions::CompletionsCmd,
        config::ConfigCmd, copy::CopyCmd, diff::DiffCmd, docs::DocsCmd, dump::DumpCmd,
        find::FindCmd, forget::ForgetCmd, init::InitCmd, key::KeyCmd, list::ListCmd, ls::LsCmd,
        merge::MergeCmd, prune::PruneCmd, repair::RepairCmd, repoinfo::RepoInfoCmd,
        restore::RestoreCmd, self_update::SelfUpdateCmd, show_config::ShowConfigCmd,
        snapshots::SnapshotCmd, tag::TagCmd,
    },
    config::RusticConfig,
};

use abscissa_core::{
    config::Override,
    tracing::log::{log, Level},
    Command, Configurable, FrameworkError, Runnable,
};
use anyhow::Result;
use clap::builder::{
    styling::{AnsiColor, Effects},
    Styles,
};
use convert_case::{Case, Casing};
use human_panic::setup_panic;

/// Rustic Subcommands
/// Subcommands need to be listed in an enum.
#[derive(clap::Parser, Command, Debug, Runnable)]
enum RusticCmd {
    /// Backup to the repository
    Backup(Box<BackupCmd>),

    /// Show raw data of files and blobs in a repository
    Cat(Box<CatCmd>),

    /// Change the repository configuration
    Config(Box<ConfigCmd>),

    /// Generate shell completions
    Completions(Box<CompletionsCmd>),

    /// Check the repository
    Check(Box<CheckCmd>),

    /// Copy snapshots to other repositories
    Copy(Box<CopyCmd>),

    /// Compare two snapshots or paths
    Diff(Box<DiffCmd>),

    /// Open the documentation
    Docs(Box<DocsCmd>),

    /// Dump the contents of a file within a snapshot to stdout
    Dump(Box<DumpCmd>),

    /// Find patterns in given snapshots
    Find(Box<FindCmd>),

    /// Remove snapshots from the repository
    Forget(Box<ForgetCmd>),

    /// Initialize a new repository
    Init(Box<InitCmd>),

    /// Manage keys for a repository
    Key(Box<KeyCmd>),

    /// List repository files by file type
    List(Box<ListCmd>),

    /// List file contents of a snapshot
    Ls(Box<LsCmd>),

    /// Merge snapshots
    Merge(Box<MergeCmd>),

    /// Show a detailed overview of the snapshots within the repository
    Snapshots(Box<SnapshotCmd>),

    /// Show the configuration which has been read from the config file(s)
    ShowConfig(Box<ShowConfigCmd>),

    /// Update to the latest stable rustic release
    #[cfg_attr(not(feature = "self-update"), clap(hide = true))]
    SelfUpdate(Box<SelfUpdateCmd>),

    /// Remove unused data or repack repository pack files
    Prune(Box<PruneCmd>),

    /// Restore (a path within) a snapshot
    Restore(Box<RestoreCmd>),

    /// Repair a snapshot or the repository index
    Repair(Box<RepairCmd>),

    /// Show general information about the repository
    Repoinfo(Box<RepoInfoCmd>),

    /// Change tags of snapshots
    Tag(Box<TagCmd>),

    /// Start a webdav server which allows to access the repository
    #[cfg(feature = "webdav")]
    Webdav(Box<WebDavCmd>),
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

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Paths used by the application
    #[clap(skip)]
    pub paths: RusticPaths,
}

impl Runnable for EntryPoint {
    fn run(&self) {
        // Set up panic hook for better error messages and logs
        setup_panic!();

        self.commands.run();
    }
}

impl Override<RusticConfig> for EntryPoint {
    fn override_config(&self, mut config: RusticConfig) -> Result<RusticConfig, FrameworkError> {
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

        let config_paths = self.paths.configs();

        // get global options from command line / env and config file
        if config.global.use_profiles.is_empty() {
            config.merge_profile("rustic", &mut merge_logs, Level::Info, config_paths)?;
        } else {
            for profile in &config.global.use_profiles.clone() {
                config.merge_profile(profile, &mut merge_logs, Level::Warn, config_paths)?;
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

/// This trait allows you to define how application configuration is loaded.
impl Configurable<RusticConfig> for EntryPoint {
    /// Apply changes to the config after it's been loaded, e.g. overriding
    /// values in a config file using command-line options.
    fn process_config(&self, config: RusticConfig) -> Result<RusticConfig, FrameworkError> {
        self.override_config(config)
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
