//! Rustic Config
//!
//! See instructions in `commands.rs` to specify the path to your
//! application's configuration file and/or command-line options
//! for specifying it.

pub(crate) mod hooks;
pub(crate) mod logging;
pub(crate) mod progress_options;

use std::{
    collections::BTreeMap,
    ffi::OsString,
    fmt::{self, Display, Formatter},
    path::PathBuf,
};

use abscissa_core::{FrameworkError, FrameworkErrorKind, config::Config, path::AbsPathBuf};
use anyhow::{Result, anyhow};
use clap::{Parser, ValueHint};
use conflate::Merge;
use directories::ProjectDirs;
use itertools::Itertools;
use jiff::{Timestamp, Zoned, tz::TimeZone};
use log::Level;
use reqwest::Url;
use rustic_core::SnapshotGroupCriterion;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
#[cfg(not(all(feature = "mount", feature = "webdav")))]
use toml::Value;

#[cfg(feature = "mount")]
use crate::commands::mount::MountCmd;
#[cfg(feature = "webdav")]
use crate::commands::webdav::WebDavCmd;

use crate::{
    commands::{backup::BackupCmd, copy::CopyCmd, forget::ForgetOptions},
    config::{hooks::Hooks, logging::LoggingOptions, progress_options::ProgressOptions},
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
    #[cfg(feature = "mount")]
    #[clap(skip)]
    pub mount: MountCmd,
    #[cfg(not(feature = "mount"))]
    #[clap(skip)]
    #[merge(skip)]
    pub mount: Option<Value>,

    /// webdav options
    #[cfg(feature = "webdav")]
    #[clap(skip)]
    pub webdav: WebDavCmd,
    #[cfg(not(feature = "webdav"))]
    #[clap(skip)]
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
    ) -> Result<(), FrameworkError> {
        let profile_filename = if profile.ends_with(".toml") {
            profile.to_string()
        } else {
            profile.to_string() + ".toml"
        };
        let paths = get_config_paths(&profile_filename);

        if let Some(path) = paths.iter().find(|path| path.exists()) {
            merge_logs.push((Level::Info, format!("using config {}", path.display())));
            let config_content = std::fs::read_to_string(AbsPathBuf::canonicalize(path)?)?;
            let config_content = if self.global.profile_substitute_env {
                subst::substitute(&config_content, &subst::Env).map_err(|e| {
                    abscissa_core::error::context::Context::new(
                        FrameworkErrorKind::ParseError,
                        Some(Box::new(e)),
                    )
                })?
            } else {
                config_content
            };
            let mut config = Self::load_toml(config_content)?;
            // sanity check
            if config.global.profile_substitute_env && config.global.use_profiles.is_empty() {
                merge_logs.push((Level::Warn, "Option `profile-substitute-env` is given without any profiles to load! Note that this option does NOT apply to the file where it is specified!".to_string()));
            }
            // if "use_profile" is defined in config file, merge the referenced profiles first
            for profile in &config.global.use_profiles.clone() {
                config.merge_profile(profile, merge_logs, Level::Warn)?;
            }
            self.merge(config);
        } else {
            let paths_string = paths.iter().map(|path| path.display()).join(", ");
            merge_logs.push((
                level_missing,
                format!("using no config file, none of these exist: {paths_string}",),
            ));
        };
        Ok(())
    }
}

/// Global options
///
/// These options are available for all commands.
#[serde_as]
#[derive(Default, Debug, Parser, Clone, Deserialize, Serialize, Merge)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct GlobalOptions {
    /// Substitute environment variables in profiles
    #[clap(long, global = true, env = "RUSTIC_PROFILE_SUBSTITUTE_ENV")]
    #[merge(strategy=conflate::bool::overwrite_false)]
    pub profile_substitute_env: bool,

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

    /// Group snapshots by any combination of host,label,paths,tags, e.g. to find the latest snapshot [default: "host,label,paths"]
    #[clap(
        long,
        short = 'g',
        global = true,
        value_name = "CRITERION",
        env = "RUSTIC_GROUP_BY"
    )]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[merge(strategy=conflate::option::overwrite_none)]
    pub group_by: Option<SnapshotGroupCriterion>,

    /// Only show what would be done without modifying anything. Does not affect read-only commands.
    #[clap(long, short = 'n', global = true, env = "RUSTIC_DRY_RUN")]
    #[merge(strategy=conflate::bool::overwrite_false)]
    pub dry_run: bool,

    /// Additional to dry run, but still issue warm-up command if configured
    #[clap(long, global = true, env = "RUSTIC_DRY_RUN_WARMUP")]
    #[merge(strategy=conflate::bool::overwrite_false)]
    pub dry_run_warmup: bool,

    /// Check if index matches pack files and read pack headers if necessary
    #[clap(long, global = true, env = "RUSTIC_CHECK_INDEX")]
    #[merge(strategy=conflate::bool::overwrite_false)]
    pub check_index: bool,

    /// Settings to customize logging
    #[clap(flatten)]
    #[serde(flatten)]
    pub logging_options: LoggingOptions,

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

    /// Push metrics to a Prometheus Pushgateway
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[clap(long, global = true, env = "RUSTIC_PROMETHEUS", value_name = "PUSHGATEWAY_URL", value_hint = ValueHint::Url)]
    #[merge(strategy=conflate::option::overwrite_none)]
    pub prometheus: Option<Url>,

    /// Authenticate to Prometheus Pushgateway using this user
    #[clap(long, value_name = "USER", env = "RUSTIC_PROMETHEUS_USER")]
    #[merge(strategy=conflate::option::overwrite_none)]
    pub prometheus_user: Option<String>,

    /// Authenticate to Prometheus Pushgateway using this password
    #[clap(long, value_name = "PASSWORD", env = "RUSTIC_PROMETHEUS_PASS")]
    #[merge(strategy=conflate::option::overwrite_none)]
    pub prometheus_pass: Option<String>,

    /// Additional labels to set to generated metrics
    #[clap(skip)]
    #[merge(strategy=conflate::btreemap::append_or_ignore)]
    pub metrics_labels: BTreeMap<String, String>,

    /// OpenTelemetry metrics endpoint (HTTP Protobuf)
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[clap(long, global = true, env = "RUSTIC_OTEL", value_name = "ENDPOINT_URL", value_hint = ValueHint::Url)]
    #[merge(strategy=conflate::option::overwrite_none)]
    pub opentelemetry: Option<Url>,

    /// Show time offsets instead of converting to system time zone
    #[clap(long, global = true, env = "RUSTIC_SHOW_TIME_OFFSET")]
    #[merge(strategy=conflate::bool::overwrite_false)]
    pub show_time_offset: bool,
}

pub fn parse_labels(s: &str) -> Result<BTreeMap<String, String>> {
    s.split(',')
        .filter_map(|s| {
            let s = s.trim();
            (!s.is_empty()).then_some(s)
        })
        .map(|s| -> Result<_> {
            let pos = s.find('=').ok_or_else(|| {
                anyhow!("invalid prometheus label definition: no `=` found in `{s}`")
            })?;
            Ok((s[..pos].to_owned(), s[pos + 1..].to_owned()))
        })
        .try_collect()
}

impl GlobalOptions {
    pub fn is_metrics_configured(&self) -> bool {
        self.prometheus.is_some() || self.opentelemetry.is_some()
    }

    pub fn format_timestamp(&self, timestamp: Timestamp) -> String {
        self.format_time(&timestamp.to_zoned(TimeZone::UTC))
            .to_string()
    }

    pub fn format_time(&self, time: &Zoned) -> impl Display {
        if self.show_time_offset {
            time.strftime("%Y-%m-%d %H:%M:%S%z")
        } else {
            let tz = TimeZone::system();
            if time.offset() == tz.to_offset(time.timestamp()) {
                time.strftime("%Y-%m-%d %H:%M:%S")
            } else {
                time.with_time_zone(tz).strftime("%Y-%m-%d %H:%M:%S*")
            }
        }
    }
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
    get_environment_config_dirs()
        .into_iter()
        .chain(
            [
                ProjectDirs::from("", "", "rustic")
                    .map(|project_dirs| project_dirs.config_dir().to_path_buf()),
                get_user_config_path(),
                get_global_config_path(),
                Some(PathBuf::from(".")),
            ]
            .into_iter()
            .flatten(),
        )
        .unique()
        .map(|path| path.join(filename))
        .collect()
}

/// Get profile directories configured through the XDG environment variables.
///
/// `XDG_CONFIG_HOME` is checked before the conventional per-user directory,
/// followed by the directories in `XDG_CONFIG_DIRS`. As required by the XDG
/// Base Directory Specification, relative paths are ignored.
fn get_environment_config_dirs() -> Vec<PathBuf> {
    environment_config_dirs(
        std::env::var_os("RUSTIC_HOME"),
        std::env::var_os("XDG_CONFIG_HOME"),
        std::env::var_os("XDG_CONFIG_DIRS"),
    )
}

fn environment_config_dirs(
    rustic_home: Option<OsString>,
    config_home: Option<OsString>,
    config_dirs: Option<OsString>,
) -> Vec<PathBuf> {
    let rustic_home = rustic_home
        .into_iter()
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .map(|path| path.join("config"));

    rustic_home
        .chain(
            config_home
                .into_iter()
                .map(PathBuf::from)
                .chain(
                    config_dirs
                        .as_deref()
                        .into_iter()
                        .flat_map(std::env::split_paths),
                )
                .filter(|path| path.is_absolute())
                .map(|path| path.join("rustic")),
        )
        .unique()
        .collect()
}

/// Get the user-managed config directory on Windows.
///
/// Besides the platform configuration directory returned by [`ProjectDirs`],
/// also look in the XDG-style location that is commonly used by command-line
/// tools on Windows.
#[cfg(target_os = "windows")]
fn get_user_config_path() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(user_config_path)
}

#[cfg(any(test, target_os = "windows"))]
fn user_config_path(user_profile: impl Into<PathBuf>) -> PathBuf {
    user_profile.into().join(".config").join("rustic")
}

/// There is no additional user-managed config directory on non-Windows
/// platforms: [`ProjectDirs`] already supplies the conventional location.
#[cfg(not(target_os = "windows"))]
fn get_user_config_path() -> Option<PathBuf> {
    None
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
    use insta::{assert_debug_snapshot, assert_snapshot};

    #[cfg(target_os = "windows")]
    fn absolute_test_path(name: &str) -> PathBuf {
        PathBuf::from(r"C:\\rustic-tests").join(name)
    }

    #[cfg(not(target_os = "windows"))]
    fn absolute_test_path(name: &str) -> PathBuf {
        PathBuf::from("/rustic-tests").join(name)
    }

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
                .insert(format!("KEY{i}"), format!("VALUE{i}"));
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

    #[test]
    fn windows_user_config_path_uses_dot_config_directory() {
        assert_eq!(
            user_config_path(r"C:\\Users\\rustic"),
            PathBuf::from(r"C:\\Users\\rustic")
                .join(".config")
                .join("rustic")
        );
    }

    #[test]
    fn environment_config_dirs_follow_xdg_order() {
        let config_home = absolute_test_path("home");
        let first_config_dir = absolute_test_path("first");
        let second_config_dir = absolute_test_path("second");
        let config_dirs =
            std::env::join_paths([first_config_dir.clone(), second_config_dir.clone()]).unwrap();

        assert_eq!(
            environment_config_dirs(
                None,
                Some(config_home.clone().into_os_string()),
                Some(config_dirs)
            ),
            [config_home, first_config_dir, second_config_dir].map(|path| path.join("rustic")),
        );
    }

    #[test]
    fn rustic_home_precedes_xdg_config_directories() {
        let rustic_home = absolute_test_path("rustic-home");
        let config_home = absolute_test_path("home");
        let shared_config_dir = absolute_test_path("shared");
        let config_dirs = std::env::join_paths([shared_config_dir.clone()]).unwrap();

        assert_eq!(
            environment_config_dirs(
                Some(rustic_home.clone().into_os_string()),
                Some(config_home.clone().into_os_string()),
                Some(config_dirs),
            ),
            [
                rustic_home.join("config"),
                config_home.join("rustic"),
                shared_config_dir.join("rustic"),
            ],
        );
    }

    #[test]
    fn environment_config_dirs_ignore_relative_entries() {
        let absolute_config_dir = absolute_test_path("shared");
        let config_dirs =
            std::env::join_paths([PathBuf::from("relative"), absolute_config_dir.clone()]).unwrap();

        assert_eq!(
            environment_config_dirs(
                Some(PathBuf::from("relative-rustic-home").into_os_string()),
                Some(PathBuf::from("relative").into_os_string()),
                Some(config_dirs)
            ),
            [absolute_config_dir.join("rustic")],
        );
    }
}
