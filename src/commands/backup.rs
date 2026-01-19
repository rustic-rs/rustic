//! `backup` subcommand

use std::collections::HashMap;
use std::fmt::Display;
use std::path::PathBuf;
use std::{collections::BTreeMap, env};

use crate::{
    Application, RUSTIC_APP,
    commands::{init::init, snapshots::fill_table},
    config::{hooks::Hooks, parse_labels},
    helpers::{bold_cell, bytes_size_to_string, table},
    repository::CliRepo,
    status_err,
};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::{Context, Result, anyhow, bail};
use clap::ValueHint;
use comfy_table::Cell;
use conflate::{Merge, MergeFrom};
use log::{debug, error, info, warn};
use rustic_core::{Excludes, StringList};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use rustic_core::{
    BackupOptions, CommandInput, ConfigOptions, IndexedIds, KeyOptions, LocalSourceFilterOptions,
    LocalSourceSaveOptions, ParentOptions, PathList, ProgressBars, Repository, SnapshotOptions,
    repofile::SnapshotFile,
};

/// `backup` subcommand
#[serde_as]
#[derive(Clone, Command, Default, Debug, clap::Parser, Serialize, Deserialize, Merge)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
// Note: using cli_sources, sources and snapshots within this struct is a hack to support serde(deny_unknown_fields)
// for deserializing the backup options from TOML
// Unfortunately we cannot work with nested flattened structures, see
// https://github.com/serde-rs/serde/issues/1547
// A drawback is that a wrongly set "snapshots = ..." won't get correct error handling and need to be manually checked, see below.
#[allow(clippy::struct_excessive_bools)]
pub struct BackupCmd {
    /// Backup source (can be specified multiple times), use - for stdin. If no source is given, uses all
    /// sources defined in the config file
    #[clap(value_name = "SOURCE", value_hint = ValueHint::AnyPath)]
    #[merge(skip)]
    #[serde(skip)]
    cli_sources: Vec<String>,

    /// Backup sources defined in the config profile by the given name (can be specified multiple times)
    #[clap(long = "name", value_name = "NAME", conflicts_with = "cli_sources")]
    #[merge(skip)]
    #[serde(skip)]
    cli_name: Vec<String>,

    #[clap(skip)]
    #[merge(skip)]
    name: Option<String>,

    /// Set filename to be used when backing up from stdin
    #[clap(long, value_name = "FILENAME", default_value = "stdin", value_hint = ValueHint::FilePath)]
    #[merge(skip)]
    stdin_filename: String,

    /// Start the given command and use its output as stdin
    #[clap(long, value_name = "COMMAND")]
    #[merge(strategy=conflate::option::overwrite_none)]
    stdin_command: Option<CommandInput>,

    /// Manually set backup path in snapshot
    #[clap(long, value_name = "PATH", value_hint = ValueHint::DirPath)]
    #[merge(strategy=conflate::option::overwrite_none)]
    as_path: Option<PathBuf>,

    /// Ignore save options
    #[clap(flatten)]
    #[serde(flatten)]
    ignore_save_opts: LocalSourceSaveOptions,

    /// Don't scan the backup source for its size - this disables ETA estimation for backup.
    #[clap(long)]
    #[merge(strategy=conflate::bool::overwrite_false)]
    pub no_scan: bool,

    /// Output generated snapshot in json format
    #[clap(long)]
    #[merge(strategy=conflate::bool::overwrite_false)]
    json: bool,

    /// Show detailed information about generated snapshot
    #[clap(long, conflicts_with = "json")]
    #[merge(strategy=conflate::bool::overwrite_false)]
    long: bool,

    /// Initialize repository, if it doesn't exist yet
    #[clap(long)]
    #[merge(strategy=conflate::bool::overwrite_false)]
    init: bool,

    /// Parent processing options
    #[clap(flatten, next_help_heading = "Options for parent processing")]
    #[serde(flatten)]
    parent_opts: ParentOptions,

    /// Exclude options
    #[clap(flatten, next_help_heading = "Exclude options")]
    #[serde(flatten)]
    excludes: Excludes,

    /// Exclude options for local source
    #[clap(flatten, next_help_heading = "Exclude options for local source")]
    #[serde(flatten)]
    ignore_filter_opts: LocalSourceFilterOptions,

    /// Snapshot options
    #[clap(flatten, next_help_heading = "Snapshot options")]
    #[serde(flatten)]
    snap_opts: SnapshotOptions,

    /// Key options (when using --init)
    #[clap(flatten, next_help_heading = "Key options (when using --init)")]
    #[serde(skip)]
    #[merge(skip)]
    key_opts: KeyOptions,

    /// Config options (when using --init)
    #[clap(flatten, next_help_heading = "Config options (when using --init)")]
    #[serde(skip)]
    #[merge(skip)]
    config_opts: ConfigOptions,

    /// Hooks to use
    #[clap(skip)]
    hooks: Hooks,

    /// Backup snapshots to generate
    #[clap(skip)]
    #[merge(strategy = merge_snapshots)]
    snapshots: Vec<Self>,

    /// Backup source, used within config file
    #[clap(skip)]
    #[merge(skip)]
    sources: Vec<String>,

    /// Job name for the metrics. Default: rustic-backup
    #[clap(long, value_name = "JOB_NAME", env = "RUSTIC_METRICS_JOB")]
    #[merge(strategy=conflate::option::overwrite_none)]
    pub metrics_job: Option<String>,

    /// Additional labels to set to generated metrics
    #[clap(long, value_name = "NAME=VALUE", value_parser = parse_labels, default_value = "")]
    #[merge(strategy=conflate::btreemap::append_or_ignore)]
    metrics_labels: BTreeMap<String, String>,
}

impl BackupCmd {
    fn validate(&self) -> Result<(), &str> {
        // manually check for a "source" field, check is not done by serde, see above.
        if !self.sources.is_empty() {
            return Err("key \"sources\" is not valid in the [backup] section!");
        }

        // manually check for a "name" field, check is not done by serde, see above.
        if self.name.is_some() {
            return Err("key \"name\" is not valid in the [backup] section!");
        }

        let snapshot_opts = &self.snapshots;
        // manually check for a "sources" field, check is not done by serde, see above.
        if snapshot_opts.iter().any(|opt| !opt.snapshots.is_empty()) {
            return Err("key \"snapshots\" is not valid in a [[backup.snapshots]] section!");
        }
        Ok(())
    }
}

/// Merge backup snapshots to generate
///
/// If a snapshot is already defined on left, use that. Else add it.
///
/// # Arguments
///
/// * `left` - Vector of backup sources
pub(crate) fn merge_snapshots(left: &mut Vec<BackupCmd>, mut right: Vec<BackupCmd>) {
    let order = |opt1: &BackupCmd, opt2: &BackupCmd| {
        opt1.name
            .cmp(&opt2.name)
            .then(opt1.sources.cmp(&opt2.sources))
    };

    left.append(&mut right);
    left.sort_by(order);
    left.dedup_by(|opt1, opt2| order(opt1, opt2).is_eq());
}

impl Runnable for BackupCmd {
    fn run(&self) {
        let config = RUSTIC_APP.config();
        if let Err(err) = config.backup.validate() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        }

        if let Err(err) = config.repository.run(|repo| self.inner_run(repo)) {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl BackupCmd {
    fn inner_run(&self, repo: CliRepo) -> Result<()> {
        let config = RUSTIC_APP.config();

        // Initialize repository if --init is set and it is not yet initialized
        let repo = if self.init && repo.config_id()?.is_none() {
            if config.global.dry_run {
                bail!(
                    "cannot initialize repository {} in dry-run mode!",
                    repo.name
                );
            }
            init(repo.0, &self.key_opts, &self.config_opts)?
        } else {
            repo.open()?
        }
        .to_indexed_ids()?;

        let hooks = self.hooks(
            &config.backup.hooks,
            "backup",
            itertools::join(&config.backup.sources, ","),
        );

        hooks.use_with(|| -> Result<_> {
            let mut is_err = false;
            for (opts, sources) in self.get_snapshots_to_backup()? {
                if let Err(err) = opts.backup_snapshot(sources.clone(), &repo) {
                    error!("error backing up {sources}: {err}");
                    is_err = true;
                }
            }
            if is_err {
                Err(anyhow!("Not all snapshots were generated successfully!"))
            } else {
                Ok(())
            }
        })
    }

    fn get_snapshots_to_backup(&self) -> Result<Vec<(Self, PathList)>> {
        let config = RUSTIC_APP.config();
        let mut config_snapshots = config
            .backup
            .snapshots
            .iter()
            .map(|opt| (opt.clone(), PathList::from_iter(&opt.sources)));

        if !self.cli_sources.is_empty() {
            let sources = PathList::from_iter(&self.cli_sources);
            let mut opts = self.clone();
            // merge Options from config file, if given
            if let Some((config_opts, _)) = config_snapshots.find(|(_, s)| s == &sources) {
                info!("merging sources={sources} section from config file");
                opts.merge(config_opts);
            }
            return Ok(vec![(opts, sources)]);
        }

        let config_snapshots: Vec<_> = config_snapshots
            // filter out using cli_name, if given
            .filter(|(opt, _)| {
                self.cli_name.is_empty()
                    || opt
                        .name
                        .as_ref()
                        .is_some_and(|name| self.cli_name.contains(name))
            })
            .map(|(opt, sources)| (self.clone().merge_from(opt), sources))
            .collect();

        if config_snapshots.is_empty() {
            bail!("no backup source given.");
        }

        info!("using backup sources from config file.");
        Ok(config_snapshots)
    }

    fn hooks(&self, hooks: &Hooks, action: &str, source: impl Display) -> Hooks {
        let mut hooks_variables =
            HashMap::from([("RUSTIC_ACTION".to_string(), action.to_string())]);

        if let Some(label) = &self.snap_opts.label {
            let _ = hooks_variables.insert("RUSTIC_BACKUP_LABEL".to_string(), label.to_string());
        }

        let source = source.to_string();
        if !source.is_empty() {
            let _ = hooks_variables.insert("RUSTIC_BACKUP_SOURCES".to_string(), source.clone());
        }

        let mut tags = StringList::default();
        tags.add_all(self.snap_opts.tags.clone());
        let tags = tags.to_string();
        if !tags.is_empty() {
            let _ = hooks_variables.insert("RUSTIC_BACKUP_TAGS".to_string(), tags);
        }

        let hooks = if action == "backup" {
            hooks.with_context("backup")
        } else {
            hooks.with_context(&format!("backup {source}"))
        };

        hooks.with_env(&hooks_variables)
    }

    fn backup_snapshot<P: ProgressBars, S: IndexedIds>(
        mut self,
        source: PathList,
        repo: &Repository<P, S>,
    ) -> Result<()> {
        let config = RUSTIC_APP.config();
        let snapshot_opts = &config.backup.snapshots;
        if let Some(path) = &self.as_path {
            // as_path only works in combination with a single target
            if source.len() > 1 {
                bail!("as-path only works with a single source!");
            }
            // merge Options from config file using as_path, if given
            if let Some(path) = path.as_os_str().to_str() {
                if let Some(idx) = snapshot_opts
                    .iter()
                    .position(|opt| opt.sources == vec![path])
                {
                    info!("merging snapshot=\"{path}\" section from config file");
                    self.merge(snapshot_opts[idx].clone());
                }
            }
        }

        // use hooks definition before merging "backup" section
        let hooks = self.hooks.clone();

        // merge "backup" section from config file, if given
        self.merge(config.backup.clone());

        let hooks = self.hooks(&hooks, "source-specific-backup", &source);

        // use global group-by if not set
        let mut parent_opts = self.parent_opts;
        parent_opts.group_by = parent_opts.group_by.or(config.global.group_by);

        let backup_opts = BackupOptions::default()
            .stdin_filename(self.stdin_filename)
            .stdin_command(self.stdin_command)
            .as_path(self.as_path)
            .parent_opts(parent_opts)
            .ignore_save_opts(self.ignore_save_opts)
            .excludes(self.excludes)
            .ignore_filter_opts(self.ignore_filter_opts)
            .no_scan(self.no_scan)
            .dry_run(config.global.dry_run);

        let snap = hooks.use_with(|| -> Result<_> {
            let source = source
                .clone()
                .sanitize()
                .with_context(|| format!("error sanitizing source=s\"{:?}\"", source))?
                .merge();
            Ok(repo.backup(&backup_opts, &source, self.snap_opts.to_snapshot()?)?)
        })?;

        if self.json {
            let mut stdout = std::io::stdout();
            serde_json::to_writer_pretty(&mut stdout, &snap)?;
        } else if self.long {
            let mut table = table();

            let add_entry = |title: &str, value: String| {
                _ = table.add_row([bold_cell(title), Cell::new(value)]);
            };
            fill_table(&snap, add_entry);

            println!("{table}");
        } else {
            let summary = snap.summary.as_ref().unwrap();
            info!(
                "Files:       {} new, {} changed, {} unchanged",
                summary.files_new, summary.files_changed, summary.files_unmodified
            );
            info!(
                "Dirs:        {} new, {} changed, {} unchanged",
                summary.dirs_new, summary.dirs_changed, summary.dirs_unmodified
            );
            debug!("Data Blobs:  {} new", summary.data_blobs);
            debug!("Tree Blobs:  {} new", summary.tree_blobs);
            info!(
                "Added to the repo: {} (raw: {})",
                bytes_size_to_string(summary.data_added_packed),
                bytes_size_to_string(summary.data_added)
            );

            info!(
                "processed {} files, {}",
                summary.total_files_processed,
                bytes_size_to_string(summary.total_bytes_processed)
            );
            info!("snapshot {} successfully saved.", snap.id);
        }

        if config.global.is_metrics_configured() {
            // Merge global metrics labels
            conflate::btreemap::append_or_ignore(
                &mut self.metrics_labels,
                config.global.metrics_labels.clone(),
            );
            if let Err(err) = publish_metrics(&snap, self.metrics_job, self.metrics_labels) {
                warn!("error pushing metrics: {err}");
            }
        }

        info!("backup of {source} done.");
        Ok(())
    }
}

#[cfg(not(any(feature = "prometheus", feature = "opentelemetry")))]
fn publish_metrics(
    snap: &SnapshotFile,
    job_name: Option<String>,
    mut labels: BTreeMap<String, String>,
) -> Result<()> {
    Err(anyhow!("metrics support is not compiled-in!"))
}

#[cfg(any(feature = "prometheus", feature = "opentelemetry"))]
fn publish_metrics(
    snap: &SnapshotFile,
    job_name: Option<String>,
    mut labels: BTreeMap<String, String>,
) -> Result<()> {
    use crate::metrics::MetricValue::*;
    use crate::metrics::{Metric, MetricsExporter};

    let summary = snap.summary.as_ref().expect("Reaching the 'push to prometheus' point should only happen for successful backups, which must have a summary set.");
    let metrics = [
        Metric {
            name: "rustic_backup_time",
            description: "Timestamp of this snapshot",
            value: Float(snap.time.timestamp().as_millisecond() as f64 / 1000.),
        },
        Metric {
            name: "rustic_backup_files_new",
            description: "New files compared to the last (i.e. parent) snapshot",
            value: Int(summary.files_new),
        },
        Metric {
            name: "rustic_backup_files_changed",
            description: "Changed files compared to the last (i.e. parent) snapshot",
            value: Int(summary.files_changed),
        },
        Metric {
            name: "rustic_backup_files_unmodified",
            description: "Unchanged files compared to the last (i.e. parent) snapshot",
            value: Int(summary.files_unmodified),
        },
        Metric {
            name: "rustic_backup_total_files_processed",
            description: "Total processed files",
            value: Int(summary.total_files_processed),
        },
        Metric {
            name: "rustic_backup_total_bytes_processed",
            description: "Total size of all processed files",
            value: Int(summary.total_bytes_processed),
        },
        Metric {
            name: "rustic_backup_dirs_new",
            description: "New directories compared to the last (i.e. parent) snapshot",
            value: Int(summary.dirs_new),
        },
        Metric {
            name: "rustic_backup_dirs_changed",
            description: "Changed directories compared to the last (i.e. parent) snapshot",
            value: Int(summary.dirs_changed),
        },
        Metric {
            name: "rustic_backup_dirs_unmodified",
            description: "Unchanged directories compared to the last (i.e. parent) snapshot",
            value: Int(summary.dirs_unmodified),
        },
        Metric {
            name: "rustic_backup_total_dirs_processed",
            description: "Total processed directories",
            value: Int(summary.total_dirs_processed),
        },
        Metric {
            name: "rustic_backup_total_dirsize_processed",
            description: "Total size of all processed dirs",
            value: Int(summary.total_dirsize_processed),
        },
        Metric {
            name: "rustic_backup_data_blobs",
            description: "Total number of data blobs added by this snapshot",
            value: Int(summary.data_blobs),
        },
        Metric {
            name: "rustic_backup_tree_blobs",
            description: "Total number of tree blobs added by this snapshot",
            value: Int(summary.tree_blobs),
        },
        Metric {
            name: "rustic_backup_data_added",
            description: "Total uncompressed bytes added by this snapshot",
            value: Int(summary.data_added),
        },
        Metric {
            name: "rustic_backup_data_added_packed",
            description: "Total bytes added to the repository by this snapshot",
            value: Int(summary.data_added_packed),
        },
        Metric {
            name: "rustic_backup_data_added_files",
            description: "Total uncompressed bytes (new/changed files) added by this snapshot",
            value: Int(summary.data_added_files),
        },
        Metric {
            name: "rustic_backup_data_added_files_packed",
            description: "Total bytes for new/changed files added to the repository by this snapshot",
            value: Int(summary.data_added_files_packed),
        },
        Metric {
            name: "rustic_backup_data_added_trees",
            description: "Total uncompressed bytes (new/changed directories) added by this snapshot",
            value: Int(summary.data_added_trees),
        },
        Metric {
            name: "rustic_backup_data_added_trees_packed",
            description: "Total bytes (new/changed directories) added to the repository by this snapshot",
            value: Int(summary.data_added_trees_packed),
        },
        Metric {
            name: "rustic_backup_backup_start",
            description: "Start time of the backup. This may differ from the snapshot `time`.",
            value: Float(summary.backup_start.timestamp().as_millisecond() as f64 / 1000.),
        },
        Metric {
            name: "rustic_backup_backup_end",
            description: "The time that the backup has been finished.",
            value: Float(summary.backup_end.timestamp().as_millisecond() as f64 / 1000.),
        },
        Metric {
            name: "rustic_backup_backup_duration",
            description: "Total duration of the backup in seconds, i.e. the time between `backup_start` and `backup_end`",
            value: Float(summary.backup_duration),
        },
        Metric {
            name: "rustic_backup_total_duration",
            description: "Total duration that the rustic command ran in seconds",
            value: Float(summary.total_duration),
        },
    ];

    _ = labels
        .entry("paths".to_string())
        .or_insert_with(|| format!("{}", snap.paths));
    _ = labels
        .entry("hostname".to_owned())
        .or_insert_with(|| snap.hostname.clone());
    _ = labels
        .entry("snapshot_label".to_string())
        .or_insert_with(|| snap.label.clone());
    _ = labels
        .entry("tags".to_string())
        .or_insert_with(|| format!("{}", snap.tags));

    let job_name = job_name.as_deref().unwrap_or("rustic_backup");
    let global_config = &RUSTIC_APP.config().global;

    #[cfg(feature = "prometheus")]
    if let Some(prometheus_endpoint) = &global_config.prometheus {
        use crate::metrics::prometheus::PrometheusExporter;

        let metrics_exporter = PrometheusExporter {
            endpoint: prometheus_endpoint.clone(),
            job_name: job_name.to_string(),
            grouping: labels.clone(),
            prometheus_user: global_config.prometheus_user.clone(),
            prometheus_pass: global_config.prometheus_pass.clone(),
        };

        metrics_exporter
            .push_metrics(metrics.as_slice())
            .context("pushing prometheus metrics")?;
    }

    #[cfg(not(feature = "prometheus"))]
    if global_config.prometheus.is_some() {
        bail!("prometheus metrics support is not compiled-in!");
    }

    #[cfg(feature = "opentelemetry")]
    if let Some(otlp_endpoint) = &global_config.opentelemetry {
        use crate::metrics::opentelemetry::OpentelemetryExporter;

        let metrics_exporter = OpentelemetryExporter {
            endpoint: otlp_endpoint.clone(),
            service_name: job_name.to_string(),
            labels: global_config.metrics_labels.clone(),
        };

        metrics_exporter
            .push_metrics(metrics.as_slice())
            .context("pushing opentelemetry metrics")?;
    }

    #[cfg(not(feature = "opentelemetry"))]
    if global_config.opentelemetry.is_some() {
        bail!("opentelemetry metrics support is not compiled-in!");
    }

    Ok(())
}
