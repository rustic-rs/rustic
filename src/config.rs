//! Rustic Config
//!
//! See instructions in `commands.rs` to specify the path to your
//! application's configuration file and/or command-line options
//! for specifying it.

pub(crate) mod progress_options;

use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use merge::Merge;

use abscissa_core::config::Config;
use abscissa_core::path::AbsPathBuf;
use abscissa_core::FrameworkError;
use clap::Parser;
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
        let mut path = ProjectDirs::from("", "", "rustic").map_or_else(
            || Path::new(".").to_path_buf(),
            |path| path.config_dir().to_path_buf(),
        );
        if !path.exists() {
            path = Path::new(".").to_path_buf();
        };
        let path = path.join(profile.to_string() + ".toml");

        if path.exists() {
            // TODO: This should be log::info! - however, the logging config
            // can be stored in the config file and is needed to initialize the logger
            eprintln!("using config {}", path.display());
            let mut config = Self::load_toml_file(AbsPathBuf::new(&path)?)?;
            // if "use_profile" is defined in config file, merge the referenced profiles first
            for profile in &config.global.use_profile.clone() {
                config.merge_profile(profile)?;
            }
            self.merge(config);
        } else {
            // TODO: This should be log::warn! - however, the logging config
            // can be stored in the config file and is needed to initialize the logger
            eprintln!("using no config file ({} doesn't exist)", path.display());
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
