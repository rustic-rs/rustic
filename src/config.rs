//! Rustic Config
//!
//! See instructions in `commands.rs` to specify the path to your
//! application's configuration file and/or command-line options
//! for specifying it.

pub(crate) mod progress_options;

use std::path::PathBuf;

use directories::ProjectDirs;

use merge::Merge;

use abscissa_core::config::Config;
use abscissa_core::path::AbsPathBuf;
use abscissa_core::FrameworkError;
use clap::Parser;
use itertools::Itertools;
use rustic_core::RepositoryOptions;
use serde::{Deserialize, Serialize};

use crate::{
    commands::{backup::BackupCmd, copy::Targets, forget::ForgetOptions},
    config::progress_options::ProgressOptions,
    filtering::SnapshotFilter,
};

/// Rustic Configuration
#[derive(Clone, Default, Debug, Parser, Deserialize, Merge)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct RusticConfig {
    #[clap(flatten, next_help_heading = "Global options")]
    pub global: GlobalOptions,

    #[clap(flatten, next_help_heading = "Repository options")]
    pub repository: RepositoryOptions,

    #[clap(flatten, next_help_heading = "Snapshot filter options")]
    pub snapshot_filter: SnapshotFilter,

    #[clap(skip)]
    pub backup: BackupCmd,

    #[clap(skip)]
    pub copy: Targets,

    #[clap(skip)]
    pub forget: ForgetOptions,
}

impl RusticConfig {
    pub fn merge_profile(&mut self, profile: &str) -> Result<(), FrameworkError> {
        let profile_filename = profile.to_string() + ".toml";
        let paths = get_config_paths(&profile_filename);

        if let Some(path) = paths.iter().find(|path| path.exists()) {
            // TODO: This should be log::info! - however, the logging config
            // can be stored in the config file and is needed to initialize the logger
            eprintln!("using config {}", path.display());
            let mut config = Self::load_toml_file(AbsPathBuf::canonicalize(path)?)?;
            // if "use_profile" is defined in config file, merge the referenced profiles first
            for profile in &config.global.use_profile.clone() {
                config.merge_profile(profile)?;
            }
            self.merge(config);
        } else {
            let paths_string = paths.iter().map(|path| path.display()).join(", ");
            // TODO: This should be log::warn! - however, the logging config
            // can be stored in the config file and is needed to initialize the logger
            eprintln!(
                "using no config file, none of these exist: {}",
                &paths_string
            );
        };
        Ok(())
    }
}

#[derive(Default, Debug, Parser, Clone, Deserialize, Serialize, Merge)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct GlobalOptions {
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
    pub use_profile: Vec<String>,

    /// Only show what would be done without modifying anything. Does not affect read-only commands
    #[clap(long, short = 'n', global = true, env = "RUSTIC_DRY_RUN")]
    #[merge(strategy = merge::bool::overwrite_false)]
    pub dry_run: bool,

    /// Use this log level [default: info]
    #[clap(long, global = true, env = "RUSTIC_LOG_LEVEL")]
    pub log_level: Option<String>,

    /// Write log messages to the given file instead of printing them.
    /// Note: warnings and errors are still additionally printed unless they are ignored by --log-level
    #[clap(long, global = true, env = "RUSTIC_LOG_FILE", value_name = "LOGFILE")]
    pub log_file: Option<PathBuf>,

    /// Settings to customize progress bars
    #[clap(flatten)]
    #[serde(flatten)]
    pub progress_options: ProgressOptions,
}

fn get_config_paths(filename: &str) -> Vec<PathBuf> {
    [
        ProjectDirs::from("", "", "rustic")
            .map(|project_dirs| project_dirs.config_dir().to_path_buf()),
        get_global_config_path(),
        Some(PathBuf::from(".")),
    ]
    .into_iter()
    .filter_map(|path| {
        path.map(|mut p| {
            p.push(filename);
            p
        })
    })
    .collect()
}

#[cfg(target_os = "windows")]
fn get_global_config_path() -> Option<PathBuf> {
    std::env::var_os("PROGRAMDATA").map(|program_data| {
        let mut path = PathBuf::from(program_data);
        path.push(r"rustic\config");
        path
    })
}

#[cfg(any(target_os = "ios", target_arch = "wasm32"))]
fn get_global_config_path() -> Option<PathBuf> {
    None
}

#[cfg(not(any(target_os = "windows", target_os = "ios", target_arch = "wasm32")))]
fn get_global_config_path() -> Option<PathBuf> {
    Some(PathBuf::from("/etc/rustic"))
}
