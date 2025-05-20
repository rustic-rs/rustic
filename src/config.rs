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

use abscissa_core::{FrameworkError, config::Config, path::AbsPathBuf};
use anyhow::{Result, anyhow, bail};
use clap::{Parser, ValueHint};
use conflate::Merge;
use directories::ProjectDirs;
use itertools::Itertools;
use log::{Level, debug};
use reqwest::Url;
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
#[serde_as]
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

    /// Push metrics to a Prometheus Pushgateway
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[clap(long, global = true, env = "RUSTIC_PROMETHEUS", value_name = "PUSHGATEWAY_URL", value_hint = ValueHint::Url)]
    #[merge(strategy=conflate::option::overwrite_none)]
    prometheus: Option<Url>,

    /// Authenticate to Prometheus Pushgateway using this user
    #[clap(long, value_name = "USER", env = "RUSTIC_PROMETHEUS_USER")]
    #[merge(strategy=conflate::option::overwrite_none)]
    prometheus_user: Option<String>,

    /// Authenticate to Prometheus Pushgateway using this password
    #[clap(long, value_name = "PASSWORD", env = "RUSTIC_PROMETHEUS_PASS")]
    #[merge(strategy=conflate::option::overwrite_none)]
    prometheus_pass: Option<String>,

    /// Additional labels to set to generated Prometheus metrics
    #[clap(skip)]
    #[merge(strategy=conflate::btreemap::append_or_ignore)]
    pub prometheus_labels: BTreeMap<String, String>,
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

#[cfg(feature = "prometheus")]
impl GlobalOptions {
    pub fn is_prometheus_configured(&self) -> bool {
        self.prometheus.is_some()
    }

    pub fn push_metrics(&self, job_name: &str, labels: BTreeMap<String, String>) -> Result<()> {
        use prometheus::{Encoder, ProtobufEncoder};
        use reqwest::{StatusCode, blocking::Client, header::CONTENT_TYPE};

        // only move on if a prometheus push url is given
        let Some(url) = &self.prometheus else {
            return Ok(());
        };

        let (full_url, encoded_metrics) = make_url_and_encoded_metrics(url, job_name, labels)?;
        debug!("using url: {full_url}");

        let mut builder = Client::new()
            .post(full_url)
            .header(CONTENT_TYPE, ProtobufEncoder::new().format_type())
            .body(encoded_metrics);

        if let Some(username) = &self.prometheus_user {
            debug!(
                "using auth {} {}",
                username,
                self.prometheus_pass.as_deref().unwrap_or("[NOT SET]")
            );
            builder = builder.basic_auth(username, self.prometheus_pass.as_ref());
        }

        let response = builder.send()?;

        match response.status() {
            StatusCode::ACCEPTED | StatusCode::OK => Ok(()),
            _ => bail!(
                "unexpected status code {} while pushing to {}",
                response.status(),
                url
            ),
        }
    }
}

#[cfg(feature = "prometheus")]
// TODO: This should be actually part of the prometheus crate, see https://github.com/tikv/rust-prometheus/issues/536
fn make_url_and_encoded_metrics(
    url: &Url,
    job: &str,
    grouping: BTreeMap<String, String>,
) -> Result<(Url, Vec<u8>)> {
    use base64::prelude::*;
    use prometheus::{Encoder, ProtobufEncoder};

    let mut url_components = vec![
        "metrics".to_string(),
        "job@base64".to_string(),
        BASE64_URL_SAFE_NO_PAD.encode(job),
    ];

    for (ln, lv) in &grouping {
        // See https://github.com/tikv/rust-prometheus/issues/535
        if !lv.is_empty() {
            // TODO: check label name
            let name = ln.to_string() + "@base64";
            url_components.push(name);
            url_components.push(BASE64_URL_SAFE_NO_PAD.encode(lv));
        }
    }
    let url = url.join(&url_components.join("/"))?;

    let encoder = ProtobufEncoder::new();
    let mut buf = Vec::new();
    for mf in prometheus::gather() {
        // Note: We don't check here for pre-existing grouping labels, as we don't set them

        // Ignore error, `no metrics` and `no name`.
        let _ = encoder.encode(&[mf], &mut buf);
    }

    Ok((url, buf))
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
    use anyhow::Result;
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

    #[cfg(feature = "prometheus")]
    #[test]
    fn test_make_url_and_encoded_metrics() -> Result<()> {
        use std::str::FromStr;

        let grouping = [
            ("abc", "xyz"),
            ("path", "/my/path"),
            ("tags", "a,b,cde"),
            ("nogroup", ""),
        ]
        .into_iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
        let (url, _) =
            make_url_and_encoded_metrics(&Url::from_str("http://host")?, "test_job", grouping)?;
        assert_eq!(
            url.to_string(),
            "http://host/metrics/job@base64/dGVzdF9qb2I/abc@base64/eHl6/path@base64/L215L3BhdGg/tags@base64/YSxiLGNkZQ"
        );
        Ok(())
    }
}
