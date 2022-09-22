use std::str::FromStr;

use anyhow::Result;
use chrono::{DateTime, Datelike, Duration, Local, Timelike};
use clap::{AppSettings, Parser};
use derivative::Derivative;
use merge::Merge;
use prettytable::{format, row, Table};
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};

use super::{progress_counter, prune, RusticConfig};
use crate::backend::{Cache, DecryptFullBackend, FileType};
use crate::repo::{
    ConfigFile, SnapshotFile, SnapshotFilter, SnapshotGroup, SnapshotGroupCriterion, StringList,
};

#[derive(Parser)]
#[clap(global_setting(AppSettings::DeriveDisplayOrder))]
pub(super) struct Opts {
    #[clap(flatten)]
    config: ConfigOpts,

    /// Also prune the repository
    #[clap(long)]
    prune: bool,

    #[clap(flatten, help_heading = "PRUNE OPTIONS (only when used with --prune)")]
    prune_opts: prune::Opts,

    /// Don't remove anything, only show what would be done
    #[clap(skip)]
    dry_run: bool,

    /// Snapshots to forget
    ids: Vec<String>,
}

#[serde_as]
#[derive(Default, Parser, Deserialize, Merge)]
#[clap(global_setting(AppSettings::DeriveDisplayOrder))]
#[serde(default, rename_all = "kebab-case")]
struct ConfigOpts {
    /// Group snapshots by any combination of host,paths,tags (default: "host,paths")
    #[clap(long, short = 'g', value_name = "CRITERION")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    group_by: Option<SnapshotGroupCriterion>,

    #[clap(flatten, help_heading = "SNAPSHOT FILTER OPTIONS")]
    #[serde(flatten)]
    filter: SnapshotFilter,

    #[clap(flatten, help_heading = "RETENTION OPTIONS")]
    #[serde(flatten)]
    keep: KeepOptions,
}

pub(super) async fn execute(
    be: &(impl DecryptFullBackend + Unpin),
    cache: Option<Cache>,
    mut opts: Opts,
    config: ConfigFile,
    config_file: RusticConfig,
) -> Result<()> {
    // merge "forget" section from config file, if given
    config_file.merge_into("forget", &mut opts.config)?;
    // merge "snapshot-filter" section from config file, if given
    config_file.merge_into("snapshot-filter", &mut opts.config.filter)?;

    opts.dry_run = opts.prune_opts.dry_run;
    let group_by = opts
        .config
        .group_by
        .unwrap_or_else(|| SnapshotGroupCriterion::from_str("host,paths").unwrap());

    let groups = match opts.ids.is_empty() {
        true => SnapshotFile::group_from_backend(be, &opts.config.filter, &group_by).await?,
        false => vec![(
            SnapshotGroup::default(),
            SnapshotFile::from_ids(be, &opts.ids).await?,
        )],
    };
    let mut forget_snaps = Vec::new();

    for (group, mut snapshots) in groups {
        if !group.is_empty() {
            println!("snapshots for {group}");
        }
        snapshots.sort_unstable_by(|sn1, sn2| sn1.cmp(sn2).reverse());
        let latest_time = snapshots[0].time;
        let mut group_keep = opts.config.keep.clone();
        let mut table = Table::new();

        let mut iter = snapshots.iter().peekable();
        let mut last = None;
        let now = Local::now();
        // snapshots that have no reason to be kept are removed. The only exception
        // is if no IDs are explicitely given and no keep option is set. In this
        // case, the default is to keep the snapshots.
        let default_keep = opts.ids.is_empty() && group_keep == KeepOptions::default();

        while let Some(sn) = iter.next() {
            let (action, reason) = {
                if sn.must_keep(now) {
                    ("keep", "snapshot".to_string())
                } else if sn.must_delete(now) {
                    forget_snaps.push(sn.id);
                    ("remove", "snapshot".to_string())
                } else if !opts.ids.is_empty() {
                    forget_snaps.push(sn.id);
                    ("remove", "id argument".to_string())
                } else {
                    match group_keep.matches(sn, last, iter.peek().is_some(), latest_time) {
                        None if default_keep => ("keep", "".to_string()),
                        None => {
                            forget_snaps.push(sn.id);
                            ("remove", "".to_string())
                        }
                        Some(reason) => ("keep", reason),
                    }
                }
            };

            let tags = sn.tags.formatln();
            let paths = sn.paths.formatln();
            let time = sn.time.format("%Y-%m-%d %H:%M:%S");
            table.add_row(row![sn.id, time, sn.hostname, tags, paths, action, reason]);

            last = Some(sn);
        }
        table.set_titles(
            row![b->"ID", b->"Time", b->"Host", b->"Tags", b->"Paths", b->"Action", br->"Reason"],
        );
        table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);

        println!();
        table.printstd();
        println!();
    }

    match (forget_snaps.is_empty(), opts.dry_run) {
        (true, _) => println!("nothing to remove"),
        (false, true) => println!(
            "would have removed the following snapshots:\n {:?}",
            forget_snaps
        ),
        (false, false) => {
            let p = progress_counter("removing snapshots...");
            be.delete_list(FileType::Snapshot, true, forget_snaps.clone(), p)
                .await?;
        }
    }

    if opts.prune {
        prune::execute(be, cache, opts.prune_opts, config, forget_snaps).await?;
    }

    Ok(())
}

#[serde_as]
#[derive(Clone, PartialEq, Derivative, Parser, Deserialize, Merge)]
#[derivative(Default)]
#[serde(default, rename_all = "kebab-case")]
pub(super) struct KeepOptions {
    /// Keep snapshots with this taglist (can be specified multiple times)
    #[clap(long, value_name = "TAG[,TAG,..]")]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[merge(strategy=merge::vec::overwrite_empty)]
    keep_tags: Vec<StringList>,

    /// Keep snapshots ids that start with ID (can be specified multiple times)
    #[clap(long = "keep-id", value_name = "ID")]
    #[merge(strategy=merge::vec::overwrite_empty)]
    keep_ids: Vec<String>,

    /// Keep the last N snapshots
    #[clap(long, short = 'l', value_name = "N", default_value = "0")]
    #[merge(strategy=merge::num::overwrite_zero)]
    keep_last: u32,

    /// Keep the last N hourly snapshots
    #[clap(long, short = 'H', value_name = "N", default_value = "0")]
    #[merge(strategy=merge::num::overwrite_zero)]
    keep_hourly: u32,

    /// Keep the last N daily snapshots
    #[clap(long, short = 'd', value_name = "N", default_value = "0")]
    #[merge(strategy=merge::num::overwrite_zero)]
    keep_daily: u32,

    /// Keep the last N weekly snapshots
    #[clap(long, short = 'w', value_name = "N", default_value = "0")]
    #[merge(strategy=merge::num::overwrite_zero)]
    keep_weekly: u32,

    /// Keep the last N monthly snapshots
    #[clap(long, short = 'm', value_name = "N", default_value = "0")]
    #[merge(strategy=merge::num::overwrite_zero)]
    keep_monthly: u32,

    /// Keep the last N yearly snapshots
    #[clap(long, short = 'y', value_name = "N", default_value = "0")]
    #[merge(strategy=merge::num::overwrite_zero)]
    keep_yearly: u32,

    /// Keep snapshots newer than DURATION relative to latest snapshot
    #[clap(long, value_name = "DURATION", default_value = "0h")]
    #[derivative(Default(value = "std::time::Duration::ZERO.into()"))]
    #[serde_as(as = "DisplayFromStr")]
    #[merge(strategy=overwrite_zero_duration)]
    keep_within: humantime::Duration,

    /// Keep hourly snapshots newer than DURATION relative to latest snapshot
    #[clap(long, value_name = "DURATION", default_value = "0h")]
    #[derivative(Default(value = "std::time::Duration::ZERO.into()"))]
    #[serde_as(as = "DisplayFromStr")]
    #[merge(strategy=overwrite_zero_duration)]
    keep_within_hourly: humantime::Duration,

    /// Keep daily snapshots newer than DURATION relative to latest snapshot
    #[clap(long, value_name = "DURATION", default_value = "0d")]
    #[derivative(Default(value = "std::time::Duration::ZERO.into()"))]
    #[serde_as(as = "DisplayFromStr")]
    #[merge(strategy=overwrite_zero_duration)]
    keep_within_daily: humantime::Duration,

    /// Keep weekly snapshots newer than DURATION relative to latest snapshot
    #[clap(long, value_name = "DURATION", default_value = "0w")]
    #[derivative(Default(value = "std::time::Duration::ZERO.into()"))]
    #[serde_as(as = "DisplayFromStr")]
    #[merge(strategy=overwrite_zero_duration)]
    keep_within_weekly: humantime::Duration,

    /// Keep monthly snapshots newer than DURATION relative to latest snapshot
    #[clap(long, value_name = "DURATION", default_value = "0m")]
    #[derivative(Default(value = "std::time::Duration::ZERO.into()"))]
    #[serde_as(as = "DisplayFromStr")]
    #[merge(strategy=overwrite_zero_duration)]
    keep_within_monthly: humantime::Duration,

    /// Keep yearly snapshots newer than DURATION relative to latest snapshot
    #[clap(long, value_name = "DURATION", default_value = "0y")]
    #[derivative(Default(value = "std::time::Duration::ZERO.into()"))]
    #[serde_as(as = "DisplayFromStr")]
    #[merge(strategy=overwrite_zero_duration)]
    keep_within_yearly: humantime::Duration,
}

fn overwrite_zero_duration(left: &mut humantime::Duration, right: humantime::Duration) {
    if *left == std::time::Duration::ZERO.into() {
        *left = right;
    }
}

fn always_false(_sn1: &SnapshotFile, _sn2: &SnapshotFile) -> bool {
    false
}

fn equal_year(sn1: &SnapshotFile, sn2: &SnapshotFile) -> bool {
    let (t1, t2) = (sn1.time, sn2.time);
    t1.year() == t2.year()
}

fn equal_month(sn1: &SnapshotFile, sn2: &SnapshotFile) -> bool {
    let (t1, t2) = (sn1.time, sn2.time);
    t1.year() == t2.year() && t1.month() == t2.month()
}

fn equal_week(sn1: &SnapshotFile, sn2: &SnapshotFile) -> bool {
    let (t1, t2) = (sn1.time, sn2.time);
    t1.year() == t2.year() && t1.iso_week().week() == t2.iso_week().week()
}

fn equal_day(sn1: &SnapshotFile, sn2: &SnapshotFile) -> bool {
    let (t1, t2) = (sn1.time, sn2.time);
    t1.year() == t2.year() && t1.ordinal() == t2.ordinal()
}

fn equal_hour(sn1: &SnapshotFile, sn2: &SnapshotFile) -> bool {
    let (t1, t2) = (sn1.time, sn2.time);
    t1.year() == t2.year() && t1.ordinal() == t2.ordinal() && t1.hour() == t2.hour()
}

impl KeepOptions {
    fn matches(
        &mut self,
        sn: &SnapshotFile,
        last: Option<&SnapshotFile>,
        has_next: bool,
        latest_time: DateTime<Local>,
    ) -> Option<String> {
        let mut keep = false;
        let mut reason = String::new();

        if self
            .keep_ids
            .iter()
            .any(|id| sn.id.to_hex().starts_with(id))
        {
            keep = true;
            reason.push_str("id\n");
        }

        if !self.keep_tags.is_empty() && sn.tags.matches(&self.keep_tags) {
            keep = true;
            reason.push_str("tags\n");
        }

        let keep_checks = [
            (
                always_false as fn(&SnapshotFile, &SnapshotFile) -> bool,
                &mut self.keep_last,
                "last",
                self.keep_within,
                "within",
            ),
            (
                equal_hour,
                &mut self.keep_hourly,
                "hourly",
                self.keep_within_hourly,
                "within hourly",
            ),
            (
                equal_day,
                &mut self.keep_daily,
                "daily",
                self.keep_within_daily,
                "within daily",
            ),
            (
                equal_week,
                &mut self.keep_weekly,
                "weekly",
                self.keep_within_weekly,
                "within weekly",
            ),
            (
                equal_month,
                &mut self.keep_monthly,
                "monthly",
                self.keep_within_monthly,
                "within monthly",
            ),
            (
                equal_year,
                &mut self.keep_yearly,
                "yearly",
                self.keep_within_yearly,
                "within yearly",
            ),
        ];

        for (check_fun, counter, reason1, within, reason2) in keep_checks {
            if !has_next || last.is_none() || !check_fun(sn, last.unwrap()) {
                if *counter > 0 {
                    *counter -= 1;
                    keep = true;
                    reason.push_str(reason1);
                    reason.push('\n');
                }
                if sn.time + Duration::from_std(*within).unwrap() > latest_time {
                    keep = true;
                    reason.push_str(reason2);
                    reason.push('\n');
                }
            }
        }

        keep.then_some(reason)
    }
}
