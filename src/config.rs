//! Rustic Config
//!
//! See instructions in `commands.rs` to specify the path to your
//! application's configuration file and/or command-line options
//! for specifying it.

pub(crate) mod hooks;
pub(crate) mod progress_options;

use std::{
    collections::BTreeMap,
    fmt::{self, Display, Formatter},
    path::PathBuf,
};

use abscissa_core::{config::Config, path::AbsPathBuf, tracing::log::Level, FrameworkError};
use anyhow::Result;
use canonical_path::CanonicalPathBuf;
use clap::{Parser, ValueHint};
use conflate::Merge;
use itertools::Itertools;
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

impl Display for RusticConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let config = toml::to_string_pretty(self)
            .unwrap_or_else(|_| "<Error serializing config>".to_string());

        write!(f, "{config}",)
    }
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
        paths: &[CanonicalPathBuf],
    ) -> Result<(), FrameworkError> {
        let paths_with_filenames: Vec<PathBuf> = paths
            .iter()
            .map(|path| {
                let mut path = (*path).clone().into_path_buf();
                path.push(profile.to_string() + ".toml");
                path
            })
            .collect();

        if let Some(path) = paths_with_filenames.iter().find(|path| path.exists()) {
            merge_logs.push((Level::Info, format!("using config {}", path.display())));

            let mut config = Self::load_toml_file(AbsPathBuf::canonicalize(path)?)?;
            // if "use_profile" is defined in config file, merge the referenced profiles first
            for profile in &config.global.use_profiles.clone() {
                config.merge_profile(profile, merge_logs, Level::Warn, paths)?;
            }
            self.merge(config);
        } else {
            let paths_string = paths_with_filenames
                .iter()
                .map(|path| path.display())
                .join(", ");
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

/// Get the path to the global config directory on Windows.
///
/// # Returns
///
/// The path to the global config directory on Windows.
/// If the environment variable `PROGRAMDATA` is not set, `None` is returned.
#[cfg(target_os = "windows")]
pub(crate) fn get_global_config_path() -> Option<PathBuf> {
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
pub(crate) fn get_global_config_path() -> Option<PathBuf> {
    None
}

/// Get the path to the global config directory on non-Windows,
/// non-iOS, non-wasm targets.
///
/// # Returns
///
/// "/etc/rustic" is returned.
#[cfg(not(any(target_os = "windows", target_os = "ios", target_arch = "wasm32")))]
pub(crate) fn get_global_config_path() -> Option<PathBuf> {
    Some(PathBuf::from("/etc/rustic"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::{assert_debug_snapshot, assert_snapshot};

    #[test]
    fn test_default_config_passes() {
        let config = RusticConfig::default();

        assert_debug_snapshot!(config);
    }

    #[test]
    fn test_default_config_display_passes() {
        let config = RusticConfig::default();

        assert_snapshot!(config);
    }

    #[test]
    fn test_global_env_roundtrip_passes() {
        let mut config = RusticConfig::default();

        for i in 0..10 {
            let _ = config
                .global
                .env
                .insert(format!("KEY{}", i), format!("VALUE{}", i));
        }

        let serialized = toml::to_string(&config).unwrap();

        // Check Serialization
        assert_snapshot!(serialized);

        let deserialized: RusticConfig = toml::from_str(&serialized).unwrap();
        // Check Deserialization and Display
        assert_snapshot!(deserialized);

        // Check Debug
        assert_debug_snapshot!(deserialized);
    }
}
