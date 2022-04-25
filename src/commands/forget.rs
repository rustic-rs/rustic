use anyhow::Result;
use chrono::{DateTime, Datelike, Duration, Local, Timelike};
use clap::Parser;
use humantime;
use prettytable::{cell, format, row, Table};

use crate::backend::{DecryptFullBackend, FileType};
use crate::repo::{SnapshotFile, SnapshotFilter, SnapshotGroupCriterion, StringList};

#[derive(Parser)]
pub(super) struct Opts {
    #[clap(flatten)]
    filter: SnapshotFilter,

    /// group snapshots by any combination of host,paths,tags
    #[clap(
        long,
        short = 'g',
        value_name = "CRITERION",
        default_value = "host,paths"
    )]
    group_by: SnapshotGroupCriterion,

    #[clap(flatten)]
    keep: KeepOptions,

    /// don't remove anything, only show what would be done
    #[clap(long, short = 'n')]
    dry_run: bool,
}

pub(super) async fn execute(be: &impl DecryptFullBackend, opts: Opts) -> Result<()> {
    let groups = SnapshotFile::group_from_backend(be, &opts.filter, &opts.group_by).await?;

    let mut forget_snaps = Vec::new();

    for (group, mut snapshots) in groups {
        if !group.is_empty() {
            println!("snapshots for {:?}", group);
        }
        snapshots.sort_unstable_by(|sn1, sn2| sn1.cmp(sn2).reverse());
        let latest_time = snapshots[0].time;
        let mut group_keep = opts.keep.clone();
        let mut table = Table::new();

        let mut iter = snapshots.iter().peekable();
        let mut last = None;

        while let Some(sn) = iter.next() {
            let (action, reason) =
                match group_keep.matches(sn, last, iter.peek().is_some(), latest_time) {
                    None => {
                        forget_snaps.push(sn.id);
                        ("remove", "".to_string())
                    }
                    Some(reason) => ("keep", reason),
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
    if opts.dry_run {
        println!(
            "would have deleted the following snapshots:\n {:?}",
            forget_snaps
        )
    } else {
        // TODO: delete in parallel
        for id in forget_snaps {
            be.remove(FileType::Snapshot, &id).await?;
        }
    }

    // TODO: Add option to call prune directly (also for the dry-run case)

    Ok(())
}

#[derive(Clone, Parser)]
struct KeepOptions {
    /// keep snapshots with this taglist (can be specified multiple times)
    #[clap(long, value_name = "TAGS")]
    keep_tags: Vec<StringList>,

    /// keep the last N snapshots
    #[clap(long, short = 'l', value_name = "N", default_value = "0")]
    keep_last: u32,

    /// keep the last N hourly snapshots
    #[clap(long, short = 'H', value_name = "N", default_value = "0")]
    keep_hourly: u32,

    /// keep the last N daily snapshots
    #[clap(long, short = 'd', value_name = "N", default_value = "0")]
    keep_daily: u32,

    /// keep the last N weekly snapshots
    #[clap(long, short = 'w', value_name = "N", default_value = "0")]
    keep_weekly: u32,

    /// keep the last N monthly snapshots
    #[clap(long, short = 'm', value_name = "N", default_value = "0")]
    keep_monthly: u32,

    /// keep the last N yearly snapshots
    #[clap(long, short = 'y', value_name = "N", default_value = "0")]
    keep_yearly: u32,

    /// keep snapshots newer than DURATION relative to latest snapshot
    #[clap(long, value_name = "DURATION", default_value = "0h")]
    keep_within: humantime::Duration,

    /// keep hourly snapshots newer than DURATION relative to latest snapshot
    #[clap(long, value_name = "DURATION", default_value = "0h")]
    keep_within_hourly: humantime::Duration,

    /// keep daily snapshots newer than DURATION relative to latest snapshot
    #[clap(long, value_name = "DURATION", default_value = "0d")]
    keep_within_daily: humantime::Duration,

    /// keep weekly snapshots newer than DURATION relative to latest snapshot
    #[clap(long, value_name = "DURATION", default_value = "0w")]
    keep_within_weekly: humantime::Duration,

    /// keep monthly snapshots newer than DURATION relative to latest snapshot
    #[clap(long, value_name = "DURATION", default_value = "0m")]
    keep_within_monthly: humantime::Duration,

    /// keep yearly snapshots newer than DURATION relative to latest snapshot
    #[clap(long, value_name = "DURATION", default_value = "0y")]
    keep_within_yearly: humantime::Duration,
}

fn equal_year(sn1: &SnapshotFile, sn2: &SnapshotFile) -> bool {
    let t1 = sn1.time;
    let t2 = sn2.time;
    t1.year() == t2.year()
}

fn equal_month(sn1: &SnapshotFile, sn2: &SnapshotFile) -> bool {
    let t1 = sn1.time;
    let t2 = sn2.time;
    t1.year() == t2.year() && t1.month() == t2.month()
}

fn equal_week(sn1: &SnapshotFile, sn2: &SnapshotFile) -> bool {
    let t1 = sn1.time;
    let t2 = sn2.time;
    t1.year() == t2.year() && t1.iso_week().week() == t2.iso_week().week()
}

fn equal_day(sn1: &SnapshotFile, sn2: &SnapshotFile) -> bool {
    let t1 = sn1.time;
    let t2 = sn2.time;
    t1.year() == t2.year() && t1.ordinal() == t2.ordinal()
}

fn equal_hour(sn1: &SnapshotFile, sn2: &SnapshotFile) -> bool {
    let t1 = sn1.time;
    let t2 = sn2.time;
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
        if !self.keep_tags.is_empty() && sn.tags.matches(&self.keep_tags) {
            keep = true;
            reason.push_str("tags\n");
        }

        if sn.time + Duration::from_std(*self.keep_within).unwrap() > latest_time {
            keep = true;
            reason.push_str("within\n");
        }

        let keep_checks = vec![
            (
                equal_hour as fn(&SnapshotFile, &SnapshotFile) -> bool,
                &mut self.keep_hourly,
                self.keep_within_hourly,
                "hourly",
            ),
            (
                equal_day,
                &mut self.keep_daily,
                self.keep_within_daily,
                "daily",
            ),
            (
                equal_week,
                &mut self.keep_weekly,
                self.keep_within_weekly,
                "weekly",
            ),
            (
                equal_month,
                &mut self.keep_monthly,
                self.keep_within_monthly,
                "monthly",
            ),
            (
                equal_year,
                &mut self.keep_yearly,
                self.keep_within_yearly,
                "yearly",
            ),
        ];

        for (check_fun, counter, within, reason_string) in keep_checks {
            if !has_next || last.is_none() || !check_fun(sn, last.unwrap()) {
                if *counter > 0 {
                    *counter -= 1;
                    keep = true;
                    reason.push_str(reason_string);
                    reason.push_str("\n");
                }
                if sn.time + Duration::from_std(*within).unwrap() > latest_time {
                    keep = true;
                    reason.push_str(reason_string);
                    reason.push_str(" within\n");
                }
            }
        }

        keep.then(|| reason)
    }
}
