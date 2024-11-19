//! Rustic Config
//!
//! See instructions in `commands.rs` to specify the path to your
//! application's configuration file and/or command-line options
//! for specifying it.

pub(crate) mod hooks;
pub(crate) mod progress_options;

use std::{collections::BTreeMap, path::PathBuf};

use abscissa_core::{config::Config, path::AbsPathBuf, FrameworkError};
use anyhow::Result;
use clap::{Parser, ValueHint};
use conflate::Merge;
use directories::ProjectDirs;
use itertools::Itertools;
use log::Level;
use serde::{Deserialize, Serialize};
#[cfg(not(all(feature = "mount", feature = "webdav")))]
use toml::Value;

#[cfg(feature = "mount")]
use crate::commands::mount::MountCmd;
#[cfg(feature = "webdav")]
use crate::commands::webdav::WebDavCmd;

use crate::{
    commands::{backup::BackupCmd, copy::CopyCmd, forget::ForgetOptions},
    config::{hooks::Hooks, progress_options::ProgressOptions},
    filtering::SnapshotFilter,
    repository::AllRepositoryOptions,
};

/// Rustic Configuration
///
/// Further documentation can be found [here](https://github.com/rustic-rs/rustic/blob/main/config/README.md).
///
/// # Example
// TODO: add example
#[derive(Clone, Default, Debug, Parser, Deserialize, Serialize, Merge)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct RusticConfig {
    /// Global options
    #[clap(flatten, next_help_heading = "Global options")]
    pub global: GlobalOptions,

    /// Repository options
    #[clap(flatten, next_help_heading = "Repository options")]
    pub repository: AllRepositoryOptions,

    /// Snapshot filter options
    #[clap(flatten, next_help_heading = "Snapshot filter options")]
    pub snapshot_filter: SnapshotFilter,

    /// Backup options
    #[clap(skip)]
    pub backup: BackupCmd,

    /// Copy options
    #[clap(skip)]
    pub copy: CopyCmd,

    /// Forget options
    #[clap(skip)]
    pub forget: ForgetOptions,

    /// mount options
    #[clap(skip)]
    #[cfg(feature = "mount")]
    pub mount: MountCmd,
    #[cfg(not(feature = "mount"))]
    #[merge(skip)]
    pub mount: Option<Value>,

    /// webdav options
    #[clap(skip)]
    #[cfg(feature = "webdav")]
    pub webdav: WebDavCmd,
    #[cfg(not(feature = "webdav"))]
    #[merge(skip)]
    pub webdav: Option<Value>,
}

impl RusticConfig {
    /// Merge a profile into the current config by reading the corresponding config file.
    /// Also recursively merge all profiles given within this config file.
    ///
    /// # Arguments
    ///
    /// * `profile` - name of the profile to merge
    /// * `merge_logs` - Vector to collect logs during merging
    /// * `level_missing` - The log level to use if this profile is missing. Recursive calls will produce a Warning.
    pub fn merge_profile(
        &mut self,
        profile: &str,
        merge_logs: &mut Vec<(Level, String)>,
        level_missing: Level,
    ) -> Result<(), FrameworkError> {
        let profile_filename = profile.to_string() + ".toml";
        let paths = get_config_paths(&profile_filename);

        if let Some(path) = paths.iter().find(|path| path.exists()) {
            merge_logs.push((Level::Info, format!("using config {}", path.display())));
            let mut config = Self::load_toml_file(AbsPathBuf::canonicalize(path)?)?;
            // if "use_profile" is defined in config file, merge the referenced profiles first
            for profile in &config.global.use_profiles.clone() {
                config.merge_profile(profile, merge_logs, Level::Warn)?;
            }
            self.merge(config);
        } else {
            let paths_string = paths.iter().map(|path| path.display()).join(", ");
            merge_logs.push((
                level_missing,
                format!(
                    "using no config file, none of these exist: {}",
                    &paths_string
                ),
            ));
        };
        Ok(())
    }
}

/// Global options
///
/// These options are available for all commands.
#[derive(Default, Debug, Parser, Clone, Deserialize, Serialize, Merge)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct GlobalOptions {
    /// Config profile to use. This parses the file `<PROFILE>.toml` in the config directory.
    /// [default: "rustic"]
    #[clap(
        short = 'P',
        long = "use-profile",
        global = true,
        value_name = "PROFILE",
        env = "RUSTIC_USE_PROFILE"
    )]
    #[merge(strategy=conflate::vec::append)]
    pub use_profiles: Vec<String>,

    /// Only show what would be done without modifying anything. Does not affect read-only commands.
    #[clap(long, short = 'n', global = true, env = "RUSTIC_DRY_RUN")]
    #[merge(strategy=conflate::bool::overwrite_false)]
    pub dry_run: bool,

    /// Check if index matches pack files and read pack headers if necessary
    #[clap(long, global = true, env = "RUSTIC_CHECK_INDEX")]
    #[merge(strategy=conflate::bool::overwrite_false)]
    pub check_index: bool,

    /// Use this log level [default: info]
    #[clap(long, global = true, env = "RUSTIC_LOG_LEVEL")]
    #[merge(strategy=conflate::option::overwrite_none)]
    pub log_level: Option<String>,

    /// Write log messages to the given file instead of printing them.
    ///
    /// # Note
    ///
    /// Warnings and errors are still additionally printed unless they are ignored by `--log-level`
    #[clap(long, global = true, env = "RUSTIC_LOG_FILE", value_name = "LOGFILE", value_hint = ValueHint::FilePath)]
    #[merge(strategy=conflate::option::overwrite_none)]
    pub log_file: Option<PathBuf>,

    /// Settings to customize progress bars
    #[clap(flatten)]
    #[serde(flatten)]
    pub progress_options: ProgressOptions,

    /// Hooks
    #[clap(skip)]
    pub hooks: Hooks,

    /// List of environment variables to set (only in config file)
    #[clap(skip)]
    #[merge(strategy = conflate::btreemap::append_or_ignore)]
    pub env: BTreeMap<String, String>,
}

/// Get the paths to the config file
///
/// # Arguments
///
/// * `filename` - name of the config file
///
/// # Returns
///
/// A vector of [`PathBuf`]s to the config files
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

/// Get the path to the global config directory on Windows.
///
/// # Returns
///
/// The path to the global config directory on Windows.
/// If the environment variable `PROGRAMDATA` is not set, `None` is returned.
#[cfg(target_os = "windows")]
fn get_global_config_path() -> Option<PathBuf> {
    std::env::var_os("PROGRAMDATA").map(|program_data| {
        let mut path = PathBuf::from(program_data);
        path.push(r"rustic\config");
        path
    })
}

/// Get the path to the global config directory on ios and wasm targets.
///
/// # Returns
///
/// `None` is returned.
#[cfg(any(target_os = "ios", target_arch = "wasm32"))]
fn get_global_config_path() -> Option<PathBuf> {
    None
}

/// Get the path to the global config directory on non-Windows,
/// non-iOS, non-wasm targets.
///
/// # Returns
///
/// "/etc/rustic" is returned.
#[cfg(not(any(target_os = "windows", target_os = "ios", target_arch = "wasm32")))]
fn get_global_config_path() -> Option<PathBuf> {
    Some(PathBuf::from("/etc/rustic"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_debug_snapshot;

    #[test]
    fn test_default_config_passes() {
        let config = RusticConfig::default();

        assert_debug_snapshot!(config);
    }

    #[test]
    fn test_global_env_deserialization_passes() {
        let config = r#"
            [global.env]
            KEY1 = "VALUE1"
            KEY2 = "VALUE2"
        "#;

        let config: RusticConfig = toml::from_str(config).unwrap();

        assert_eq!(config.global.env.len(), 2);
        assert_eq!(config.global.env.get("KEY1"), Some(&"VALUE1".to_string()));
        assert_eq!(config.global.env.get("KEY2"), Some(&"VALUE2".to_string()));
    }
}
