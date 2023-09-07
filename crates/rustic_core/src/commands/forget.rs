//! `forget` subcommand

use chrono::{DateTime, Datelike, Duration, Local, Timelike};
use derivative::Derivative;
use derive_setters::Setters;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

use crate::{
    error::RusticResult,
    id::Id,
    progress::ProgressBars,
    repofile::snapshotfile::{SnapshotGroup, SnapshotGroupCriterion},
    repofile::{SnapshotFile, StringList},
    repository::{Open, Repository},
};

type CheckFunction = fn(&SnapshotFile, &SnapshotFile) -> bool;

#[derive(Debug, Serialize)]
/// A newtype for `[Vec<ForgetGroup>]`
pub struct ForgetGroups(pub Vec<ForgetGroup>);

#[derive(Debug, Serialize)]
/// All snapshots of a group with group and forget information
pub struct ForgetGroup {
    /// The group
    pub group: SnapshotGroup,
    /// The list of snapshots within this group
    pub snapshots: Vec<ForgetSnapshot>,
}

#[derive(Debug, Serialize)]
/// This struct enhances `[SnapshotFile]` with the attributes `keep` and `reasons` which indicates if the snapshot should be kept and why.
pub struct ForgetSnapshot {
    /// The snapshot
    pub snapshot: SnapshotFile,
    /// Whether it should be kept
    pub keep: bool,
    /// reason(s) for keeping / not keeping the snapshot
    pub reasons: Vec<String>,
}

impl ForgetGroups {
    /// Turn `ForgetGroups` into the list of all snapshot IDs to remove.
    pub fn into_forget_ids(self) -> Vec<Id> {
        self.0
            .into_iter()
            .flat_map(|fg| {
                fg.snapshots
                    .into_iter()
                    .filter_map(|fsn| (!fsn.keep).then_some(fsn.snapshot.id))
            })
            .collect()
    }
}

/// Get the list of snapshots to forget.
///
/// # Type Parameters
///
/// * `P` - The progress bar type.
/// * `S` - The state the repository is in.
///
/// # Arguments
///
/// * `repo` - The repository to use
/// * `keep` - The keep options to use
/// * `group_by` - The criterion to group snapshots by
/// * `filter` - The filter to apply to the snapshots
///
/// # Returns
///
/// The list of snapshot groups with the corresponding snapshots and forget information
pub(crate) fn get_forget_snapshots<P: ProgressBars, S: Open>(
    repo: &Repository<P, S>,
    keep: &KeepOptions,
    group_by: SnapshotGroupCriterion,
    filter: impl FnMut(&SnapshotFile) -> bool,
) -> RusticResult<ForgetGroups> {
    let now = Local::now();

    let groups = repo
        .get_snapshot_group(&[], group_by, filter)?
        .into_iter()
        .map(|(group, snapshots)| ForgetGroup {
            group,
            snapshots: keep.apply(snapshots, now),
        })
        .collect();

    Ok(ForgetGroups(groups))
}

#[cfg_attr(feature = "clap", derive(clap::Parser))]
#[cfg_attr(feature = "merge", derive(merge::Merge))]
#[serde_as]
#[derive(Clone, Debug, PartialEq, Eq, Derivative, Deserialize, Setters)]
#[derivative(Default)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
#[setters(into)]
#[non_exhaustive]
/// Options which snapshots should be kept. Used by the `forget` command.
pub struct KeepOptions {
    /// Keep snapshots with this taglist (can be specified multiple times)
    #[cfg_attr(feature = "clap", clap(long, value_name = "TAG[,TAG,..]"))]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[cfg_attr(feature = "merge", merge(strategy=merge::vec::overwrite_empty))]
    pub keep_tags: Vec<StringList>,

    /// Keep snapshots ids that start with ID (can be specified multiple times)
    #[cfg_attr(feature = "clap", clap(long = "keep-id", value_name = "ID"))]
    #[cfg_attr(feature = "merge", merge(strategy=merge::vec::overwrite_empty))]
    pub keep_ids: Vec<String>,

    /// Keep the last N snapshots (N == -1: keep all snapshots)
    #[cfg_attr(
        feature = "clap", 
        clap(long, short = 'l', value_name = "N", default_value = "0", allow_hyphen_values = true, value_parser = clap::value_parser!(i32).range(-1..))
    )]
    #[cfg_attr(feature = "merge", merge(strategy=merge::num::overwrite_zero))]
    pub keep_last: i32,

    /// Keep the last N hourly snapshots (N == -1: keep all hourly snapshots)
    #[cfg_attr(
        feature = "clap", 
        clap(long, short = 'H', value_name = "N", default_value = "0", allow_hyphen_values = true, value_parser = clap::value_parser!(i32).range(-1..))
    )]
    #[cfg_attr(feature = "merge", merge(strategy=merge::num::overwrite_zero))]
    pub keep_hourly: i32,

    /// Keep the last N daily snapshots (N == -1: keep all daily snapshots)
    #[cfg_attr(
        feature = "clap", 
        clap(long, short = 'd', value_name = "N", default_value = "0", allow_hyphen_values = true, value_parser = clap::value_parser!(i32).range(-1..))
    )]
    #[cfg_attr(feature = "merge", merge(strategy=merge::num::overwrite_zero))]
    pub keep_daily: i32,

    /// Keep the last N weekly snapshots (N == -1: keep all weekly snapshots)
    #[cfg_attr(
        feature = "clap",
        clap(long, short = 'w', value_name = "N", default_value = "0", allow_hyphen_values = true, value_parser = clap::value_parser!(i32).range(-1..))
    )]
    #[cfg_attr(feature = "merge", merge(strategy=merge::num::overwrite_zero))]
    pub keep_weekly: i32,

    /// Keep the last N monthly snapshots (N == -1: keep all monthly snapshots)
    #[cfg_attr(
        feature = "clap", 
        clap(long, short = 'm', value_name = "N", default_value = "0", allow_hyphen_values = true, value_parser = clap::value_parser!(i32).range(-1..))
    )]
    #[cfg_attr(feature = "merge", merge(strategy=merge::num::overwrite_zero))]
    pub keep_monthly: i32,

    /// Keep the last N quarter-yearly snapshots (N == -1: keep all quarter-yearly snapshots)
    #[cfg_attr(
        feature = "clap", 
        clap(long, value_name = "N", default_value = "0", allow_hyphen_values = true, value_parser = clap::value_parser!(i32).range(-1..))
    )]
    #[cfg_attr(feature = "merge", merge(strategy=merge::num::overwrite_zero))]
    pub keep_quarter_yearly: i32,

    /// Keep the last N half-yearly snapshots (N == -1: keep all half-yearly snapshots)
    #[cfg_attr(
        feature = "clap", 
        clap(long, value_name = "N", default_value = "0", allow_hyphen_values = true, value_parser = clap::value_parser!(i32).range(-1..))
    )]
    #[cfg_attr(feature = "merge", merge(strategy=merge::num::overwrite_zero))]
    pub keep_half_yearly: i32,

    /// Keep the last N yearly snapshots (N == -1: keep all yearly snapshots)
    #[cfg_attr(
        feature = "clap", 
        clap(long, short = 'y', value_name = "N", default_value = "0", allow_hyphen_values = true, value_parser = clap::value_parser!(i32).range(-1..))
    )]
    #[cfg_attr(feature = "merge", merge(strategy=merge::num::overwrite_zero))]
    pub keep_yearly: i32,

    /// Keep snapshots newer than DURATION relative to latest snapshot
    #[cfg_attr(
        feature = "clap",
        clap(long, value_name = "DURATION", default_value = "0h")
    )]
    #[derivative(Default(value = "std::time::Duration::ZERO.into()"))]
    #[serde_as(as = "DisplayFromStr")]
    #[cfg_attr(feature = "merge", merge(strategy=overwrite_zero_duration))]
    pub keep_within: humantime::Duration,

    /// Keep hourly snapshots newer than DURATION relative to latest snapshot
    #[cfg_attr(
        feature = "clap",
        clap(long, value_name = "DURATION", default_value = "0h")
    )]
    #[derivative(Default(value = "std::time::Duration::ZERO.into()"))]
    #[serde_as(as = "DisplayFromStr")]
    #[cfg_attr(feature = "merge", merge(strategy=overwrite_zero_duration))]
    pub keep_within_hourly: humantime::Duration,

    /// Keep daily snapshots newer than DURATION relative to latest snapshot
    #[cfg_attr(
        feature = "clap",
        clap(long, value_name = "DURATION", default_value = "0d")
    )]
    #[derivative(Default(value = "std::time::Duration::ZERO.into()"))]
    #[serde_as(as = "DisplayFromStr")]
    #[cfg_attr(feature = "merge", merge(strategy=overwrite_zero_duration))]
    pub keep_within_daily: humantime::Duration,

    /// Keep weekly snapshots newer than DURATION relative to latest snapshot
    #[cfg_attr(
        feature = "clap",
        clap(long, value_name = "DURATION", default_value = "0w")
    )]
    #[derivative(Default(value = "std::time::Duration::ZERO.into()"))]
    #[serde_as(as = "DisplayFromStr")]
    #[cfg_attr(feature = "merge", merge(strategy=overwrite_zero_duration))]
    pub keep_within_weekly: humantime::Duration,

    /// Keep monthly snapshots newer than DURATION relative to latest snapshot
    #[cfg_attr(
        feature = "clap",
        clap(long, value_name = "DURATION", default_value = "0m")
    )]
    #[derivative(Default(value = "std::time::Duration::ZERO.into()"))]
    #[serde_as(as = "DisplayFromStr")]
    #[cfg_attr(feature = "merge", merge(strategy=overwrite_zero_duration))]
    pub keep_within_monthly: humantime::Duration,

    /// Keep quarter-yearly snapshots newer than DURATION relative to latest snapshot
    #[cfg_attr(
        feature = "clap",
        clap(long, value_name = "DURATION", default_value = "0y")
    )]
    #[derivative(Default(value = "std::time::Duration::ZERO.into()"))]
    #[serde_as(as = "DisplayFromStr")]
    #[cfg_attr(feature = "merge", merge(strategy=overwrite_zero_duration))]
    pub keep_within_quarter_yearly: humantime::Duration,

    /// Keep half-yearly snapshots newer than DURATION relative to latest snapshot
    #[cfg_attr(
        feature = "clap",
        clap(long, value_name = "DURATION", default_value = "0y")
    )]
    #[derivative(Default(value = "std::time::Duration::ZERO.into()"))]
    #[serde_as(as = "DisplayFromStr")]
    #[cfg_attr(feature = "merge", merge(strategy=overwrite_zero_duration))]
    pub keep_within_half_yearly: humantime::Duration,

    /// Keep yearly snapshots newer than DURATION relative to latest snapshot
    #[cfg_attr(
        feature = "clap",
        clap(long, value_name = "DURATION", default_value = "0y")
    )]
    #[derivative(Default(value = "std::time::Duration::ZERO.into()"))]
    #[serde_as(as = "DisplayFromStr")]
    #[cfg_attr(feature = "merge", merge(strategy=overwrite_zero_duration))]
    pub keep_within_yearly: humantime::Duration,
}

/// Overwrite the value of `left` with `right` if `left` is zero.
///
/// This is used to overwrite the default values of `KeepOptions` with the values from the config file.
///
/// # Arguments
///
/// * `left` - The value to overwrite
/// * `right` - The value to overwrite with
///
/// # Example
///
/// ```
/// use rustic_core::commands::forget::overwrite_zero_duration;
/// use humantime::Duration;
///
/// let mut left = "0s".parse::<humantime::Duration>().unwrap().into();
/// let right = "60s".parse::<humantime::Duration>().unwrap().into();
/// overwrite_zero_duration(&mut left, right);
/// assert_eq!(left, "60s".parse::<humantime::Duration>().unwrap().into());
/// ```
#[cfg(feature = "merge")]
fn overwrite_zero_duration(left: &mut humantime::Duration, right: humantime::Duration) {
    if *left == std::time::Duration::ZERO.into() {
        *left = right;
    }
}

/// Always return false
///
/// # Arguments
///
/// * `_sn1` - The first snapshot
/// * `_sn2` - The second snapshot
const fn always_false(_sn1: &SnapshotFile, _sn2: &SnapshotFile) -> bool {
    false
}

/// Evaluate the year of the given snapshots
///
/// # Arguments
///
/// * `sn1` - The first snapshot
/// * `sn2` - The second snapshot
///
/// # Returns
///
/// Whether the year of the snapshots is equal
fn equal_year(sn1: &SnapshotFile, sn2: &SnapshotFile) -> bool {
    let (t1, t2) = (sn1.time, sn2.time);
    t1.year() == t2.year()
}

/// Evaluate the half year of the given snapshots
///
/// # Arguments
///
/// * `sn1` - The first snapshot
/// * `sn2` - The second snapshot
///
/// # Returns
///
/// Whether the half year of the snapshots is equal
fn equal_half_year(sn1: &SnapshotFile, sn2: &SnapshotFile) -> bool {
    let (t1, t2) = (sn1.time, sn2.time);
    t1.year() == t2.year() && t1.month0() / 6 == t2.month0() / 6
}

/// Evaluate the quarter year of the given snapshots
///
/// # Arguments
///
/// * `sn1` - The first snapshot
/// * `sn2` - The second snapshot
///
/// # Returns
///
/// Whether the quarter year of the snapshots is equal
fn equal_quarter_year(sn1: &SnapshotFile, sn2: &SnapshotFile) -> bool {
    let (t1, t2) = (sn1.time, sn2.time);
    t1.year() == t2.year() && t1.month0() / 3 == t2.month0() / 3
}

/// Evaluate the month of the given snapshots
///
/// # Arguments
///
/// * `sn1` - The first snapshot
/// * `sn2` - The second snapshot
///
/// # Returns
///
/// Whether the month of the snapshots is equal
fn equal_month(sn1: &SnapshotFile, sn2: &SnapshotFile) -> bool {
    let (t1, t2) = (sn1.time, sn2.time);
    t1.year() == t2.year() && t1.month() == t2.month()
}

/// Evaluate the week of the given snapshots
///
/// # Arguments
///
/// * `sn1` - The first snapshot
/// * `sn2` - The second snapshot
///
/// # Returns
///
/// Whether the week of the snapshots is equal
fn equal_week(sn1: &SnapshotFile, sn2: &SnapshotFile) -> bool {
    let (t1, t2) = (sn1.time, sn2.time);
    t1.year() == t2.year() && t1.iso_week().week() == t2.iso_week().week()
}

/// Evaluate the day of the given snapshots
///
/// # Arguments
///
/// * `sn1` - The first snapshot
/// * `sn2` - The second snapshot
///
/// # Returns
///
/// Whether the day of the snapshots is equal
fn equal_day(sn1: &SnapshotFile, sn2: &SnapshotFile) -> bool {
    let (t1, t2) = (sn1.time, sn2.time);
    t1.year() == t2.year() && t1.ordinal() == t2.ordinal()
}

/// Evaluate the hours of the given snapshots
///
/// # Arguments
///
/// * `sn1` - The first snapshot
/// * `sn2` - The second snapshot
///
/// # Returns
///
/// Whether the hours of the snapshots are equal
fn equal_hour(sn1: &SnapshotFile, sn2: &SnapshotFile) -> bool {
    let (t1, t2) = (sn1.time, sn2.time);
    t1.year() == t2.year() && t1.ordinal() == t2.ordinal() && t1.hour() == t2.hour()
}

impl KeepOptions {
    /// Check if the given snapshot matches the keep options.
    ///
    /// # Arguments
    ///
    /// * `sn` - The snapshot to check
    /// * `last` - The last snapshot
    /// * `has_next` - Whether there is a next snapshot
    /// * `latest_time` - The time of the latest snapshot
    ///
    /// # Returns
    ///
    /// The list of reasons why the snapshot should be kept
    fn matches(
        &mut self,
        sn: &SnapshotFile,
        last: Option<&SnapshotFile>,
        has_next: bool,
        latest_time: DateTime<Local>,
    ) -> Vec<&str> {
        let mut reason = Vec::new();

        let snapshot_id_hex = sn.id.to_hex();
        if self
            .keep_ids
            .iter()
            .any(|id| snapshot_id_hex.starts_with(id))
        {
            reason.push("id");
        }

        if !self.keep_tags.is_empty() && sn.tags.matches(&self.keep_tags) {
            reason.push("tags");
        }

        let keep_checks: [(CheckFunction, &mut i32, &str, humantime::Duration, &str); 8] = [
            (
                always_false,
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
                equal_quarter_year,
                &mut self.keep_quarter_yearly,
                "quarter-yearly",
                self.keep_within_quarter_yearly,
                "within quarter-yearly",
            ),
            (
                equal_half_year,
                &mut self.keep_half_yearly,
                "half-yearly",
                self.keep_within_half_yearly,
                "within half-yearly",
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
                if *counter != 0 {
                    reason.push(reason1);
                    if *counter > 0 {
                        *counter -= 1;
                    }
                }
                if sn.time + Duration::from_std(*within).unwrap() > latest_time {
                    reason.push(reason2);
                }
            }
        }
        reason
    }

    /// Apply the `[KeepOptions]` to the given list of [`SnapshotFile`]s returning the corresponding
    /// list of [`ForgetSnapshot`]s
    ///
    /// # Arguments
    ///
    /// * `snapshots` - The list of snapshots to apply the options to
    /// * `now` - The current time
    ///
    /// # Returns
    ///
    /// The list of snapshots with the attribute `keep` set to `true` if the snapshot should be kept and
    /// `reasons` set to the list of reasons why the snapshot should be kept
    pub fn apply(
        &self,
        mut snapshots: Vec<SnapshotFile>,
        now: DateTime<Local>,
    ) -> Vec<ForgetSnapshot> {
        let mut group_keep = self.clone();
        let mut snaps = Vec::new();
        if snapshots.is_empty() {
            return snaps;
        }

        snapshots.sort_unstable_by(|sn1, sn2| sn1.cmp(sn2).reverse());
        let latest_time = snapshots[0].time;
        let mut last = None;

        let mut iter = snapshots.into_iter().peekable();

        while let Some(sn) = iter.next() {
            let (keep, reasons) = {
                if sn.must_keep(now) {
                    (true, vec!["snapshot"])
                } else if sn.must_delete(now) {
                    (false, vec!["snapshot"])
                } else {
                    let reasons =
                        group_keep.matches(&sn, last.as_ref(), iter.peek().is_some(), latest_time);
                    let keep = !reasons.is_empty();
                    (keep, reasons)
                }
            };
            last = Some(sn.clone());

            snaps.push(ForgetSnapshot {
                snapshot: sn,
                keep,
                reasons: reasons.iter().map(ToString::to_string).collect(),
            });
        }
        snaps
    }
}
