use crate::error::RhaiErrorKinds;

use bytesize::ByteSize;
use derive_more::derive::Display;
use log::warn;
use rustic_core::{repofile::SnapshotFile, StringList};
use std::{
    error::Error,
    fmt::{Debug, Display},
    str::FromStr,
};

use cached::proc_macro::cached;
use chrono::{DateTime, Local, NaiveTime};
use conflate::Merge;
use rhai::{serde::to_dynamic, Dynamic, Engine, FnPtr, AST};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

/// A function to filter snapshots
///
/// The function is called with a [`SnapshotFile`] and must return a boolean.
#[derive(Clone, Debug)]
pub(crate) struct SnapshotFn(FnPtr, AST);

impl FromStr for SnapshotFn {
    type Err = RhaiErrorKinds;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let engine = Engine::new();
        let ast = engine.compile(s)?;
        let func = engine.eval_ast::<FnPtr>(&ast)?;
        Ok(Self(func, ast))
    }
}

#[cached(key = "String", convert = r#"{ s.to_string() }"#, size = 1)]
fn string_to_fn(s: &str) -> Option<SnapshotFn> {
    match SnapshotFn::from_str(s) {
        Ok(filter_fn) => Some(filter_fn),
        Err(err) => {
            warn!("Error evaluating filter-fn {s}: {err}",);
            None
        }
    }
}

impl SnapshotFn {
    /// Call the function with a [`SnapshotFile`]
    ///
    /// The function must return a boolean.
    ///
    /// # Errors
    ///
    // TODO!: add errors!
    fn call<T: Clone + Send + Sync + 'static>(
        &self,
        sn: &SnapshotFile,
    ) -> Result<T, Box<dyn Error>> {
        let engine = Engine::new();
        let sn: Dynamic = to_dynamic(sn)?;
        Ok(self.0.call::<T>(&engine, &self.1, (sn,))?)
    }
}

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize, Merge, clap::Parser)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct SnapshotFilter {
    /// Hostname to filter (can be specified multiple times)
    #[clap(long = "filter-host", global = true, value_name = "HOSTNAME")]
    #[merge(strategy=conflate::vec::overwrite_empty)]
    filter_hosts: Vec<String>,

    /// Label to filter (can be specified multiple times)
    #[clap(long = "filter-label", global = true, value_name = "LABEL")]
    #[merge(strategy=conflate::vec::overwrite_empty)]
    filter_labels: Vec<String>,

    /// Path list to filter (can be specified multiple times)
    #[clap(long, global = true, value_name = "PATH[,PATH,..]")]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[merge(strategy=conflate::vec::overwrite_empty)]
    filter_paths: Vec<StringList>,

    /// Path list to filter exactly (no superset) as given (can be specified multiple times)
    #[clap(long, global = true, value_name = "PATH[,PATH,..]")]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[merge(strategy=conflate::vec::overwrite_empty)]
    filter_paths_exact: Vec<StringList>,

    /// Tag list to filter (can be specified multiple times)
    #[clap(long, global = true, value_name = "TAG[,TAG,..]")]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[merge(strategy=conflate::vec::overwrite_empty)]
    filter_tags: Vec<StringList>,

    /// Tag list to filter exactly (no superset) as given (can be specified multiple times)
    #[clap(long, global = true, value_name = "TAG[,TAG,..]")]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[merge(strategy=conflate::vec::overwrite_empty)]
    filter_tags_exact: Vec<StringList>,

    /// Only use snapshots which are taken after the given given date/time
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[clap(long, global = true, value_name = "DATE(TIME)")]
    #[merge(strategy=conflate::option::overwrite_none)]
    filter_after: Option<AfterDate>,

    /// Only use snapshots which are taken before the given given date/time
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[clap(long, global = true, value_name = "DATE(TIME)")]
    #[merge(strategy=conflate::option::overwrite_none)]
    filter_before: Option<BeforeDate>,

    /// Only use snapshots with total size in given range
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[clap(long, global = true, value_name = "SIZE")]
    #[merge(strategy=conflate::option::overwrite_none)]
    filter_size: Option<SizeRange>,

    /// Only use snapshots with size added to the repo in given range
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[clap(long, global = true, value_name = "SIZE")]
    #[merge(strategy=conflate::option::overwrite_none)]
    filter_size_added: Option<SizeRange>,

    /// Function to filter snapshots
    #[clap(long, global = true, value_name = "FUNC")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[merge(strategy=conflate::option::overwrite_none)]
    filter_fn: Option<String>,
}

impl SnapshotFilter {
    /// Check if a [`SnapshotFile`] matches the filter
    ///
    /// # Arguments
    ///
    /// * `snapshot` - The snapshot to check
    ///
    /// # Returns
    ///
    /// `true` if the snapshot matches the filter, `false` otherwise
    #[must_use]
    pub fn matches(&self, snapshot: &SnapshotFile) -> bool {
        if let Some(filter_fn) = &self.filter_fn {
            if let Some(func) = string_to_fn(filter_fn) {
                match func.call::<bool>(snapshot) {
                    Ok(result) => {
                        if !result {
                            return false;
                        }
                    }
                    Err(err) => {
                        warn!(
                            "Error evaluating filter-fn for snapshot {}: {err}",
                            snapshot.id
                        );
                    }
                }
            }
        }

        if matches!(&self.filter_after, Some(after) if snapshot.time <= after.0)
            || matches!(&self.filter_before, Some(before) if snapshot.time >= before.0)
            || matches!((&self.filter_size,&snapshot.summary), (Some(size),Some(summary)) if !size.matches(summary.total_bytes_processed))
            || matches!((&self.filter_size_added,&snapshot.summary), (Some(size),Some(summary)) if !size.matches(summary.data_added))
        {
            return false;
        }

        snapshot.paths.matches(&self.filter_paths)
            && (self.filter_paths_exact.is_empty()
                || self.filter_paths_exact.contains(&snapshot.paths))
            && snapshot.tags.matches(&self.filter_tags)
            && (self.filter_tags_exact.is_empty()
                || self.filter_tags_exact.contains(&snapshot.tags))
            && (self.filter_hosts.is_empty() || self.filter_hosts.contains(&snapshot.hostname))
            && (self.filter_labels.is_empty() || self.filter_labels.contains(&snapshot.label))
    }
}

#[derive(Debug, Clone, Display)]
struct AfterDate(DateTime<Local>);

impl FromStr for AfterDate {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let before_midnight = NaiveTime::from_hms_nano_opt(23, 59, 59, 999_999_999).unwrap();
        let datetime = dateparser::parse_with(s, &Local, before_midnight)?;
        Ok(Self(datetime.into()))
    }
}

#[derive(Debug, Clone, Display)]
struct BeforeDate(DateTime<Local>);

impl FromStr for BeforeDate {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let midnight = NaiveTime::from_hms_opt(0, 0, 0).unwrap();
        let datetime = dateparser::parse_with(s, &Local, midnight)?;
        Ok(Self(datetime.into()))
    }
}

#[derive(Debug, Clone)]
struct SizeRange {
    from: Option<ByteSize>,
    to: Option<ByteSize>,
}

impl SizeRange {
    fn matches(&self, size: u64) -> bool {
        !matches!(self.from, Some(from) if size < from.0)
            && !matches!(self.to, Some(to) if size > to.0)
    }
}

fn parse_size(s: &str) -> Result<Option<ByteSize>, String> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    Ok(Some(s.parse()?))
}

impl FromStr for SizeRange {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (from, to) = match s.split_once("..") {
            Some((s1, s2)) => (parse_size(s1)?, parse_size(s2)?),
            None => (parse_size(s)?, None),
        };
        Ok(Self { from, to })
    }
}

impl Display for SizeRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(from) = self.from {
            f.write_str(&from.to_string_as(true))?;
        }
        f.write_str("..")?;
        if let Some(to) = self.to {
            f.write_str(&to.to_string_as(true))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("..", None, None)]
    #[case("10", Some(10), None)]
    #[case("..10k", None, Some(10_000))]
    #[case("1MB..", Some(1_000_000), None)]
    #[case("10 .. 20 ", Some(10), Some(20))]
    #[case(" 2G ", Some(2_000_000_000), None)]
    fn size_range_from_str(
        #[case] input: SizeRange,
        #[case] from: Option<u64>,
        #[case] to: Option<u64>,
    ) {
        assert_eq!(input.from.map(|v| v.0), from);
        assert_eq!(input.to.map(|v| v.0), to);
    }
}
