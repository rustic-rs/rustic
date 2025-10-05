#[cfg(feature = "rhai")]
use crate::error::RhaiErrorKinds;

#[cfg(feature = "rhai")]
use std::error::Error;
use std::{
    fmt::{Debug, Display},
    str::FromStr,
};

#[cfg(feature = "jq")]
use anyhow::{anyhow, bail};
use bytesize::ByteSize;
use derive_more::derive::Display;
use log::warn;
use rustic_core::{StringList, repofile::SnapshotFile};

use cached::proc_macro::cached;
use chrono::{DateTime, Local, NaiveTime};
use conflate::Merge;

#[cfg(feature = "jq")]
use jaq_core::{
    Compiler, Ctx, Filter, Native, RcIter,
    load::{Arena, File, Loader},
};
#[cfg(feature = "jq")]
use jaq_json::Val;
#[cfg(feature = "rhai")]
use rhai::{AST, Dynamic, Engine, FnPtr, serde::to_dynamic};
use serde::{Deserialize, Serialize};
#[cfg(feature = "jq")]
use serde_json::Value;
use serde_with::{DisplayFromStr, serde_as};

/// A function to filter snapshots
///
/// The function is called with a [`SnapshotFile`] and must return a boolean.
#[cfg(feature = "rhai")]
#[derive(Clone, Debug)]
pub(crate) struct SnapshotFn(FnPtr, AST);

#[cfg(feature = "rhai")]
impl FromStr for SnapshotFn {
    type Err = RhaiErrorKinds;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let engine = Engine::new();
        let ast = engine.compile(s)?;
        let func = engine.eval_ast::<FnPtr>(&ast)?;
        Ok(Self(func, ast))
    }
}

#[cfg(feature = "rhai")]
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

#[cfg(feature = "rhai")]
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

#[cfg(feature = "jq")]
#[derive(Clone)]
pub(crate) struct SnapshotJq(Filter<Native<Val>>);

#[cfg(feature = "jq")]
impl FromStr for SnapshotJq {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let programm = File { code: s, path: () };
        let loader = Loader::new(jaq_std::defs().chain(jaq_json::defs()));
        let arena = Arena::default();
        let modules = loader
            .load(&arena, programm)
            .map_err(|errs| anyhow!("errors loading modules in jq: {errs:?}"))?;
        let filter = Compiler::<_, Native<_>>::default()
            .with_funs(jaq_std::funs().chain(jaq_json::funs()))
            .compile(modules)
            .map_err(|errs| anyhow!("errors during compiling filters in jq: {errs:?}"))?;

        Ok(Self(filter))
    }
}

#[cfg(feature = "jq")]
impl SnapshotJq {
    fn call(&self, snap: &SnapshotFile) -> Result<bool, anyhow::Error> {
        let input = serde_json::to_value(snap)?;

        let inputs = RcIter::new(core::iter::empty());
        let res = self.0.run((Ctx::new([], &inputs), Val::from(input))).next();

        match res {
            Some(Ok(val)) => {
                let val: Value = val.into();
                match val.as_bool() {
                    Some(true) => Ok(true),
                    Some(false) => Ok(false),
                    None => bail!("expression does not return bool"),
                }
            }
            _ => bail!("expression does not return bool"),
        }
    }
}

#[cfg(feature = "jq")]
#[cached(key = "String", convert = r#"{ s.to_string() }"#, size = 1)]
fn string_to_jq(s: &str) -> Option<SnapshotJq> {
    match SnapshotJq::from_str(s) {
        Ok(filter_jq) => Some(filter_jq),
        Err(err) => {
            warn!("Error evaluating filter-fn {s}: {err}",);
            None
        }
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

    /// Only use the last COUNT snapshots for each group
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[clap(long, global = true, value_name = "COUNT")]
    #[merge(strategy=conflate::option::overwrite_none)]
    filter_last: Option<usize>,

    /// Function to filter snapshots
    #[cfg(feature = "rhai")]
    #[clap(long, global = true, value_name = "FUNC")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[merge(strategy=conflate::option::overwrite_none)]
    filter_fn: Option<String>,

    /// jq to filter snapshots
    #[cfg(feature = "jq")]
    #[clap(long, global = true, value_name = "JQ")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[merge(strategy=conflate::option::overwrite_none)]
    filter_jq: Option<String>,
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
        #[cfg(feature = "rhai")]
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
                        return false;
                    }
                }
            }
        }
        #[cfg(feature = "jq")]
        if let Some(filter_jq) = &self.filter_jq {
            if let Some(jq) = string_to_jq(filter_jq) {
                match jq.call(snapshot) {
                    Ok(result) => {
                        if !result {
                            return false;
                        }
                    }
                    Err(err) => {
                        warn!(
                            "Error evaluating filter-jq for snapshot {}: {err}",
                            snapshot.id
                        );
                        return false;
                    }
                }
            }
        }

        // For the `Option`s we check if the option is set and the condition is not matched. In this case we can early return false.
        if matches!(&self.filter_after, Some(after) if !after.matches(snapshot.time))
            || matches!(&self.filter_before, Some(before) if !before.matches(snapshot.time))
            || matches!((&self.filter_size,&snapshot.summary), (Some(size),Some(summary)) if !size.matches(summary.total_bytes_processed))
            || matches!((&self.filter_size_added,&snapshot.summary), (Some(size),Some(summary)) if !size.matches(summary.data_added))
        {
            return false;
        }

        // For the the `Vec`s we have two possibilities:
        // - There exists a suitable matches method on the snapshot item
        //   (this automatically handles empty filter correctly):
        snapshot.paths.matches(&self.filter_paths)
            && snapshot.tags.matches(&self.filter_tags)
        //  - manually check if the snapshot item is contained in the `Vec`
        //    but only if the `Vec` is not empty.
        //    If it is empty, no condition is given.
            && (self.filter_paths_exact.is_empty()
                || self.filter_paths_exact.contains(&snapshot.paths))
            && (self.filter_tags_exact.is_empty()
                || self.filter_tags_exact.contains(&snapshot.tags))
            && (self.filter_hosts.is_empty() || self.filter_hosts.contains(&snapshot.hostname))
            && (self.filter_labels.is_empty() || self.filter_labels.contains(&snapshot.label))
    }

    pub fn post_process(&self, snapshots: &mut Vec<SnapshotFile>) {
        snapshots.sort_unstable();
        if let Some(last) = self.filter_last {
            let count = snapshots.len();
            if last < count {
                let new = snapshots.split_off(count - last);
                let _ = std::mem::replace(snapshots, new);
            }
        }
    }
}

#[derive(Debug, Clone, Display)]
struct AfterDate(DateTime<Local>);

impl AfterDate {
    fn matches(&self, datetime: DateTime<Local>) -> bool {
        self.0 < datetime
    }
}

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

impl BeforeDate {
    fn matches(&self, datetime: DateTime<Local>) -> bool {
        datetime < self.0
    }
}

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
        // The matches-expression is only true if the `Option` is `Some` and the size is smaller than from.
        // Hence, !matches is true either if `self.from` is `None` or if the size >= the values
        !matches!(self.from, Some(from) if size < from.0)
        // same logic here, but smaller and greater swapped.
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
            Display::fmt(&from.display(), f)?;
        }
        f.write_str("..")?;
        if let Some(to) = self.to {
            Display::fmt(&to.display(), f)?;
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
    #[case("1 MB .. 1 GiB", Some(1_000_000), Some(1_073_741_824))]
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
