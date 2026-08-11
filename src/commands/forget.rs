//! `forget` subcommand

use std::{collections::BTreeMap, time::Instant};

use crate::repository::{OpenRepo, get_grouped_snapshots};
use crate::{Application, RUSTIC_APP, RusticConfig, helpers::table_with_titles, status_err};

use abscissa_core::{Command, FrameworkError, Runnable};
use abscissa_core::{Shutdown, config::Override};
use anyhow::Result;
use conflate::Merge;
use jiff::Zoned;
use log::{info, warn};
use rustic_core::repofile::RusticTime;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};

use crate::{
    commands::prune::{PruneCmd, PruneMetrics},
    filtering::SnapshotFilter,
};

use rustic_core::{ForgetGroups, ForgetSnapshot, KeepOptions, SnapshotGroupCriterion};

/// `forget` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(super) struct ForgetCmd {
    /// Snapshots to forget. If none is given, use filter options to filter from all snapshots
    ///
    /// Snapshots can be identified the following ways: "01a2b3c4" or "latest" or "latest~N" (N >= 0)
    #[clap(value_name = "ID")]
    ids: Vec<String>,

    /// Set the date/time (e.g. "2021-01-21) to use when evaluating retention rules; can be used to test the rules (default: now)
    #[clap(long,value_parser = RusticTime::parse_system)]
    pub forget_time: Option<Zoned>,

    /// Show infos in json format
    #[clap(long)]
    json: bool,

    /// Forget options
    #[clap(flatten)]
    config: ForgetOptions,

    /// Prune options (only when used with --prune)
    #[clap(
        flatten,
        next_help_heading = "PRUNE OPTIONS (only when used with --prune)"
    )]
    prune_opts: PruneCmd,
}

impl Override<RusticConfig> for ForgetCmd {
    // Process the given command line options, overriding settings from
    // a configuration file using explicit flags taken from command-line
    // arguments.
    fn override_config(&self, mut config: RusticConfig) -> Result<RusticConfig, FrameworkError> {
        let mut self_config = self.config.clone();
        // merge "forget" section from config file, if given
        self_config.merge(config.forget);
        // merge "snapshot-filter" section from config file, if given
        self_config.filter.merge(config.snapshot_filter.clone());
        config.forget = self_config;
        Ok(config)
    }
}

/// Forget options
#[serde_as]
#[derive(Clone, Default, Debug, clap::Parser, Serialize, Deserialize, Merge)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct ForgetOptions {
    /// Group snapshots by any combination of host,label,paths,tags (default: "host,label,paths")
    #[clap(long, short = 'g', value_name = "CRITERION")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[merge(strategy=conflate::option::overwrite_none)]
    group_by: Option<SnapshotGroupCriterion>,

    /// Also prune the repository
    #[clap(long)]
    #[merge(strategy=conflate::bool::overwrite_false)]
    prune: bool,

    /// Snapshot filter options
    #[clap(flatten, next_help_heading = "Snapshot filter options")]
    #[serde(flatten)]
    filter: SnapshotFilter,

    /// Retention options
    #[clap(flatten, next_help_heading = "Retention options")]
    #[serde(flatten)]
    keep: KeepOptions,
}

#[cfg_attr(
    not(any(feature = "prometheus", feature = "opentelemetry")),
    allow(dead_code)
)]
#[derive(Clone, Copy)]
struct RetentionMetric {
    reason: &'static str,
    name: &'static str,
    description: &'static str,
}

const RETENTION_METRICS: &[RetentionMetric] = &[
    RetentionMetric {
        reason: "id",
        name: "rustic_forget_snapshots_kept_id",
        description: "Snapshots kept by the `id` retention rule",
    },
    RetentionMetric {
        reason: "tags",
        name: "rustic_forget_snapshots_kept_tags",
        description: "Snapshots kept by the `tags` retention rule",
    },
    RetentionMetric {
        reason: "last",
        name: "rustic_forget_snapshots_kept_last",
        description: "Snapshots kept by the `last` retention rule",
    },
    RetentionMetric {
        reason: "minutely",
        name: "rustic_forget_snapshots_kept_minutely",
        description: "Snapshots kept by the `minutely` retention rule",
    },
    RetentionMetric {
        reason: "hourly",
        name: "rustic_forget_snapshots_kept_hourly",
        description: "Snapshots kept by the `hourly` retention rule",
    },
    RetentionMetric {
        reason: "daily",
        name: "rustic_forget_snapshots_kept_daily",
        description: "Snapshots kept by the `daily` retention rule",
    },
    RetentionMetric {
        reason: "weekly",
        name: "rustic_forget_snapshots_kept_weekly",
        description: "Snapshots kept by the `weekly` retention rule",
    },
    RetentionMetric {
        reason: "monthly",
        name: "rustic_forget_snapshots_kept_monthly",
        description: "Snapshots kept by the `monthly` retention rule",
    },
    RetentionMetric {
        reason: "quarter-yearly",
        name: "rustic_forget_snapshots_kept_quarter_yearly",
        description: "Snapshots kept by the `quarter-yearly` retention rule",
    },
    RetentionMetric {
        reason: "half-yearly",
        name: "rustic_forget_snapshots_kept_half_yearly",
        description: "Snapshots kept by the `half-yearly` retention rule",
    },
    RetentionMetric {
        reason: "yearly",
        name: "rustic_forget_snapshots_kept_yearly",
        description: "Snapshots kept by the `yearly` retention rule",
    },
    RetentionMetric {
        reason: "within",
        name: "rustic_forget_snapshots_kept_within",
        description: "Snapshots kept by the `within` retention rule",
    },
    RetentionMetric {
        reason: "within minutely",
        name: "rustic_forget_snapshots_kept_within_minutely",
        description: "Snapshots kept by the `within minutely` retention rule",
    },
    RetentionMetric {
        reason: "within hourly",
        name: "rustic_forget_snapshots_kept_within_hourly",
        description: "Snapshots kept by the `within hourly` retention rule",
    },
    RetentionMetric {
        reason: "within daily",
        name: "rustic_forget_snapshots_kept_within_daily",
        description: "Snapshots kept by the `within daily` retention rule",
    },
    RetentionMetric {
        reason: "within weekly",
        name: "rustic_forget_snapshots_kept_within_weekly",
        description: "Snapshots kept by the `within weekly` retention rule",
    },
    RetentionMetric {
        reason: "within monthly",
        name: "rustic_forget_snapshots_kept_within_monthly",
        description: "Snapshots kept by the `within monthly` retention rule",
    },
    RetentionMetric {
        reason: "within quarter-yearly",
        name: "rustic_forget_snapshots_kept_within_quarter_yearly",
        description: "Snapshots kept by the `within quarter-yearly` retention rule",
    },
    RetentionMetric {
        reason: "within half-yearly",
        name: "rustic_forget_snapshots_kept_within_half_yearly",
        description: "Snapshots kept by the `within half-yearly` retention rule",
    },
    RetentionMetric {
        reason: "within yearly",
        name: "rustic_forget_snapshots_kept_within_yearly",
        description: "Snapshots kept by the `within yearly` retention rule",
    },
    RetentionMetric {
        reason: "snapshot",
        name: "rustic_forget_snapshots_kept_snapshot",
        description: "Snapshots kept by their snapshot deletion policy",
    },
];

#[derive(Default)]
struct SnapshotMetrics {
    kept_by_reason: BTreeMap<&'static str, u64>,
}

#[cfg_attr(
    not(any(feature = "prometheus", feature = "opentelemetry")),
    allow(dead_code)
)]
struct PruneRunMetrics {
    start: f64,
    end: f64,
    duration: f64,
    summary: PruneMetrics,
}

#[cfg_attr(
    not(any(feature = "prometheus", feature = "opentelemetry")),
    allow(dead_code)
)]
struct ForgetRunMetrics {
    time: f64,
    forget_start: f64,
    forget_end: f64,
    forget_duration: f64,
    total_duration: f64,
    snapshots_total: u64,
    snapshots_removed: u64,
    snapshots_kept: u64,
    snapshot_metrics: SnapshotMetrics,
    prune: Option<PruneRunMetrics>,
}

fn unix_timestamp(time: &Zoned) -> f64 {
    time.timestamp().as_millisecond() as f64 / 1000.
}

fn collect_snapshot_metrics(groups: &ForgetGroups) -> SnapshotMetrics {
    collect_snapshot_metrics_from_snapshots(
        groups
            .0
            .iter()
            .flat_map(|group| group.items.iter())
            .map(|snapshot| (snapshot.keep, snapshot.reasons.as_slice())),
    )
}

fn collect_snapshot_metrics_from_snapshots<'a>(
    snapshots: impl IntoIterator<Item = (bool, &'a [String])>,
) -> SnapshotMetrics {
    let mut metrics = SnapshotMetrics::default();

    for (keep, reasons) in snapshots {
        if !keep {
            continue;
        }

        for reason in reasons {
            if let Some(metric) = RETENTION_METRICS
                .iter()
                .find(|metric| metric.reason == reason)
            {
                *metrics.kept_by_reason.entry(metric.name).or_default() += 1;
            }
        }
    }

    metrics
}

impl Runnable for ForgetCmd {
    fn run(&self) {
        if let Err(err) = RUSTIC_APP
            .config()
            .repository
            .run_open(|repo| self.inner_run(repo))
        {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl ForgetCmd {
    /// be careful about self vs `RUSTIC_APP.config()` usage
    /// only the `RUSTIC_APP.config()` involves the TOML and ENV merged configurations
    /// see <https://github.com/rustic-rs/rustic/issues/1242>
    fn inner_run(&self, repo: OpenRepo) -> Result<()> {
        let config = RUSTIC_APP.config();
        let command_start = Zoned::now();
        let command_timer = Instant::now();
        // Dry runs and JSON output do not complete the normal forget operation, so publishing
        // removal gauges for them would be misleading.
        let metrics_requested =
            config.global.is_metrics_configured() && !config.global.dry_run && !self.json;
        let snapshots_total = if metrics_requested {
            match repo.get_all_snapshots() {
                Ok(snapshots) => Some(u64::try_from(snapshots.len()).unwrap_or(u64::MAX)),
                Err(err) => {
                    warn!("error collecting forget metrics: {err}");
                    None
                }
            }
        } else {
            None
        };

        let group_by = config
            .forget
            .group_by
            .or(config.global.group_by)
            .unwrap_or_default();

        if let Some(time) = &self.forget_time {
            info!("using time: {time}");
        }
        let now = self.forget_time.clone().unwrap_or_else(Zoned::now);

        let groups = if self.ids.is_empty() {
            ForgetGroups::from_grouped_snapshots_with_retention(
                get_grouped_snapshots(&repo, group_by, &[])?,
                &config.forget.keep,
                &now,
            )?
        } else {
            ForgetGroups::from_snapshots(
                repo.get_snapshots_from_strs(&self.ids, |sn| config.snapshot_filter.matches(sn))?,
                &now,
            )
        };

        if self.json {
            let mut stdout = std::io::stdout();
            serde_json::to_writer(&mut stdout, &groups)?;
        } else {
            print_groups(&groups);
        }

        let snapshot_metrics = snapshots_total.map(|_| collect_snapshot_metrics(&groups));
        let forget_snaps = groups.into_forget_ids();
        let snapshots_removed = u64::try_from(forget_snaps.len()).unwrap_or(u64::MAX);
        let forget_start = Zoned::now();
        let forget_timer = Instant::now();

        match (forget_snaps.is_empty(), config.global.dry_run, self.json) {
            (true, _, false) => info!("nothing to remove"),
            (false, true, false) => {
                info!("would have removed {} snapshots.", forget_snaps.len());
            }
            (false, false, _) => {
                repo.delete_snapshots(&forget_snaps)?;
            }
            (_, _, true) => {}
        }

        let forget_end = Zoned::now();
        let forget_duration = forget_timer.elapsed().as_secs_f64();
        let prune = if config.forget.prune {
            let mut prune_opts = self.prune_opts.clone();
            prune_opts.opts.ignore_snaps = forget_snaps;
            let prune_start = Zoned::now();
            let prune_timer = Instant::now();
            let summary = prune_opts.inner_run(&repo)?;
            Some(PruneRunMetrics {
                start: unix_timestamp(&prune_start),
                end: unix_timestamp(&Zoned::now()),
                duration: prune_timer.elapsed().as_secs_f64(),
                summary,
            })
        } else {
            None
        };

        if let (Some(snapshot_metrics), Some(snapshots_total)) = (snapshot_metrics, snapshots_total)
        {
            let metrics = ForgetRunMetrics {
                time: unix_timestamp(&command_start),
                forget_start: unix_timestamp(&forget_start),
                forget_end: unix_timestamp(&forget_end),
                forget_duration,
                total_duration: command_timer.elapsed().as_secs_f64(),
                snapshots_total,
                snapshots_removed,
                snapshots_kept: snapshots_total.saturating_sub(snapshots_removed),
                snapshot_metrics,
                prune,
            };
            if let Err(err) = publish_forget_metrics(&metrics, &config.global.metrics_labels) {
                warn!("error pushing metrics: {err}");
            }
        }

        Ok(())
    }
}

#[cfg(not(any(feature = "prometheus", feature = "opentelemetry")))]
fn publish_forget_metrics(
    _metrics: &ForgetRunMetrics,
    _labels: &BTreeMap<String, String>,
) -> Result<()> {
    Err(anyhow::anyhow!("metrics support is not compiled-in!"))
}

#[cfg(any(feature = "prometheus", feature = "opentelemetry"))]
fn publish_forget_metrics(
    forget: &ForgetRunMetrics,
    labels: &BTreeMap<String, String>,
) -> Result<()> {
    use crate::metrics::MetricValue::*;
    use crate::metrics::{Metric, MetricsExporter};
    use anyhow::Context;

    let mut metrics = vec![
        Metric {
            name: "rustic_forget_time",
            description: "Unix timestamp when the forget command started",
            value: Float(forget.time),
        },
        Metric {
            name: "rustic_forget_forget_start",
            description: "Unix timestamp when the forget phase started",
            value: Float(forget.forget_start),
        },
        Metric {
            name: "rustic_forget_forget_end",
            description: "Unix timestamp when the forget phase completed",
            value: Float(forget.forget_end),
        },
        Metric {
            name: "rustic_forget_forget_duration",
            description: "Duration of the forget phase in seconds",
            value: Float(forget.forget_duration),
        },
        Metric {
            name: "rustic_forget_total_duration",
            description: "Total duration of the successful forget command in seconds",
            value: Float(forget.total_duration),
        },
        Metric {
            name: "rustic_forget_snapshots_total",
            description: "Total snapshots in the repository before this forget run",
            value: Int(forget.snapshots_total),
        },
        Metric {
            name: "rustic_forget_snapshots_removed",
            description: "Snapshots deleted by this forget run",
            value: Int(forget.snapshots_removed),
        },
        Metric {
            name: "rustic_forget_snapshots_kept",
            description: "Snapshots remaining after this forget run",
            value: Int(forget.snapshots_kept),
        },
    ];

    for retention in RETENTION_METRICS {
        metrics.push(Metric {
            name: retention.name,
            description: retention.description,
            value: Int(forget
                .snapshot_metrics
                .kept_by_reason
                .get(retention.name)
                .copied()
                .unwrap_or_default()),
        });
    }

    if let Some(prune) = &forget.prune {
        let summary = prune.summary;
        metrics.extend([
            Metric {
                name: "rustic_forget_prune_start",
                description: "Unix timestamp when the prune phase started",
                value: Float(prune.start),
            },
            Metric {
                name: "rustic_forget_prune_end",
                description: "Unix timestamp when the prune phase completed",
                value: Float(prune.end),
            },
            Metric {
                name: "rustic_forget_prune_duration",
                description: "Duration of the prune phase in seconds",
                value: Float(prune.duration),
            },
            // The prune plan reports packed blob lengths, not raw/uncompressed byte lengths.
            // Keep the raw `rustic_forget_data_removed*` gauges absent until core exposes those
            // values, instead of presenting packed bytes as raw bytes.
            Metric {
                name: "rustic_forget_data_removed_packed",
                description: "Compressed and encrypted blob bytes removed from the active index by the completed prune plan; this excludes pack headers and may not yet be physically deleted",
                value: Int(summary.data_removed_packed + summary.tree_removed_packed),
            },
            Metric {
                name: "rustic_forget_data_removed_files_packed",
                description: "Compressed and encrypted data-blob bytes removed from the active index by the completed prune plan; this may not yet be physically deleted",
                value: Int(summary.data_removed_packed),
            },
            Metric {
                name: "rustic_forget_data_removed_trees_packed",
                description: "Compressed and encrypted tree-blob bytes removed from the active index by the completed prune plan; this may not yet be physically deleted",
                value: Int(summary.tree_removed_packed),
            },
            Metric {
                name: "rustic_forget_data_blobs_removed",
                description: "Data blobs removed from the active index by the completed prune plan",
                value: Int(summary.data_blobs_removed),
            },
            Metric {
                name: "rustic_forget_tree_blobs_removed",
                description: "Tree blobs removed from the active index by the completed prune plan",
                value: Int(summary.tree_blobs_removed),
            },
            Metric {
                name: "rustic_forget_packs_removed",
                description: "Fully unreferenced pack candidates identified by the completed prune plan; --keep-pack can retain candidates and non-instant prune marks selected packs for later deletion",
                value: Int(summary.packs_unreferenced),
            },
            Metric {
                name: "rustic_forget_packs_rewritten",
                description: "Packs rewritten by the completed prune plan",
                value: Int(summary.packs_rewritten),
            },
            Metric {
                name: "rustic_forget_packs_kept",
                description: "Unmarked packs left untouched by the completed prune plan",
                value: Int(summary.packs_kept),
            },
        ]);
    }

    let global_config = &RUSTIC_APP.config().global;

    #[cfg(feature = "prometheus")]
    if let Some(prometheus_endpoint) = &global_config.prometheus {
        use crate::metrics::prometheus::PrometheusExporter;

        let metrics_exporter = PrometheusExporter {
            endpoint: prometheus_endpoint.clone(),
            job_name: "rustic_forget".to_string(),
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
        anyhow::bail!("prometheus metrics support is not compiled-in!");
    }

    #[cfg(feature = "opentelemetry")]
    if let Some(otlp_endpoint) = &global_config.opentelemetry {
        use crate::metrics::opentelemetry::OpentelemetryExporter;

        let metrics_exporter = OpentelemetryExporter {
            endpoint: otlp_endpoint.clone(),
            service_name: "rustic_forget".to_string(),
            labels: global_config.metrics_labels.clone(),
        };

        metrics_exporter
            .push_metrics(metrics.as_slice())
            .context("pushing opentelemetry metrics")?;
    }

    #[cfg(not(feature = "opentelemetry"))]
    if global_config.opentelemetry.is_some() {
        anyhow::bail!("opentelemetry metrics support is not compiled-in!");
    }

    Ok(())
}

/// Print groups to stdout
///
/// # Arguments
///
/// * `groups` - forget groups to print
fn print_groups(groups: &ForgetGroups) {
    let config = RUSTIC_APP.config();
    for group in &groups.0 {
        let mut table = table_with_titles([
            "ID", "Time", "Host", "Label", "Tags", "Paths", "Action", "Reason",
        ]);

        for ForgetSnapshot {
            snapshot: sn,
            keep,
            reasons,
        } in &group.items
        {
            let time = config.global.format_time(&sn.time).to_string();
            let tags = sn.tags.formatln();
            let paths = sn.paths.formatln();
            let action = if *keep { "keep" } else { "remove" };
            let reason = reasons.join("\n");
            _ = table.add_row([
                &sn.id.to_string(),
                &time,
                &sn.hostname,
                &sn.label,
                &tags,
                &paths,
                action,
                &reason,
            ]);
        }

        if !group.group_key.is_empty() {
            info!("snapshots for {}:\n{table}", group.group_key);
        } else {
            info!("snapshots:\n{table}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retention_metrics_count_each_keep_reason_and_skip_removed_snapshots() {
        let retained = vec![
            "last".to_string(),
            "daily".to_string(),
            "within daily".to_string(),
        ];
        let retained_by_policy = vec!["snapshot".to_string()];
        let removed = vec!["unchanged".to_string()];

        let metrics = collect_snapshot_metrics_from_snapshots([
            (true, retained.as_slice()),
            (true, retained_by_policy.as_slice()),
            (false, removed.as_slice()),
        ]);

        assert_eq!(
            metrics
                .kept_by_reason
                .get("rustic_forget_snapshots_kept_last"),
            Some(&1)
        );
        assert_eq!(
            metrics
                .kept_by_reason
                .get("rustic_forget_snapshots_kept_daily"),
            Some(&1)
        );
        assert_eq!(
            metrics
                .kept_by_reason
                .get("rustic_forget_snapshots_kept_within_daily"),
            Some(&1)
        );
        assert_eq!(
            metrics
                .kept_by_reason
                .get("rustic_forget_snapshots_kept_snapshot"),
            Some(&1)
        );
        assert!(
            !metrics
                .kept_by_reason
                .contains_key("rustic_forget_snapshots_kept_unchanged")
        );
    }

    #[test]
    fn retention_metrics_cover_every_current_core_reason() {
        for reason in [
            "id",
            "tags",
            "last",
            "minutely",
            "hourly",
            "daily",
            "weekly",
            "monthly",
            "quarter-yearly",
            "half-yearly",
            "yearly",
            "within",
            "within minutely",
            "within hourly",
            "within daily",
            "within weekly",
            "within monthly",
            "within quarter-yearly",
            "within half-yearly",
            "within yearly",
            "snapshot",
        ] {
            assert!(
                RETENTION_METRICS
                    .iter()
                    .any(|metric| metric.reason == reason)
            );
        }
    }
}
