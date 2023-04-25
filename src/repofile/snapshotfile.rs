use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;
use std::{cmp::Ordering, fmt::Display};

use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, Duration, Local};
use clap::Parser;
use derivative::Derivative;
use dunce::canonicalize;
use gethostname::gethostname;
use indicatif::ProgressBar;
use itertools::Itertools;
use log::*;
use merge::Merge;
use path_dedot::ParseDot;
use rhai::serde::to_dynamic;
use rhai::{Dynamic, Engine, FnPtr, AST};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DeserializeFromStr, DisplayFromStr};

use super::Id;
use crate::backend::{DecryptReadBackend, FileType, RepoFile};
use crate::repository::parse_command;

#[serde_as]
#[derive(Clone, Default, Debug, Parser, Deserialize, Merge)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct SnapshotOptions {
    /// Label snapshot with given label
    #[clap(long, value_name = "LABEL")]
    label: Option<String>,

    /// Tags to add to snapshot (can be specified multiple times)
    #[clap(long, value_name = "TAG[,TAG,..]")]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[merge(strategy = merge::vec::overwrite_empty)]
    tag: Vec<StringList>,

    /// Add description to snapshot
    #[clap(long, value_name = "DESCRIPTION")]
    description: Option<String>,

    /// Add description to snapshot from file
    #[clap(long, value_name = "FILE", conflicts_with = "description")]
    description_from: Option<PathBuf>,

    /// Mark snapshot as uneraseable
    #[clap(long, conflicts_with = "delete_after")]
    #[merge(strategy = merge::bool::overwrite_false)]
    delete_never: bool,

    /// Mark snapshot to be deleted after given duration (e.g. 10d)
    #[clap(long, value_name = "DURATION")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    delete_after: Option<humantime::Duration>,

    /// Set the host name manually
    #[clap(long, value_name = "NAME")]
    host: Option<String>,
}

/// This is an extended version of the summaryOutput structure of restic in
/// restic/internal/ui/backup$/json.go
#[derive(Debug, Clone, Serialize, Deserialize, Derivative)]
#[derivative(Default)]
pub struct SnapshotSummary {
    pub files_new: u64,
    pub files_changed: u64,
    pub files_unmodified: u64,
    pub dirs_new: u64,
    pub dirs_changed: u64,
    pub dirs_unmodified: u64,
    pub data_blobs: u64,
    pub tree_blobs: u64,
    pub data_added: u64,
    pub data_added_packed: u64,
    pub data_added_files: u64,
    pub data_added_files_packed: u64,
    pub data_added_trees: u64,
    pub data_added_trees_packed: u64,
    pub total_files_processed: u64,
    pub total_dirs_processed: u64,
    pub total_bytes_processed: u64,
    pub total_dirsize_processed: u64,
    pub total_duration: f64, // in seconds

    pub command: String,
    #[derivative(Default(value = "Local::now()"))]
    pub backup_start: DateTime<Local>,
    #[derivative(Default(value = "Local::now()"))]
    pub backup_end: DateTime<Local>,
    pub backup_duration: f64, // in seconds
}

impl SnapshotSummary {
    pub fn finalize(&mut self, snap_time: DateTime<Local>) -> Result<()> {
        let end_time = Local::now();
        self.backup_duration = (end_time - self.backup_start).to_std()?.as_secs_f64();
        self.total_duration = (end_time - snap_time).to_std()?.as_secs_f64();
        self.backup_end = end_time;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Derivative)]
#[derivative(Default)]
pub enum DeleteOption {
    #[derivative(Default)]
    NotSet,
    Never,
    After(DateTime<Local>),
}

impl DeleteOption {
    fn is_not_set(&self) -> bool {
        matches!(self, Self::NotSet)
    }
}

#[serde_with::apply(Option => #[serde(default, skip_serializing_if = "Option::is_none")])]
#[derive(Debug, Clone, Serialize, Deserialize, Derivative)]
#[derivative(Default)]
pub struct SnapshotFile {
    #[derivative(Default(value = "Local::now()"))]
    pub time: DateTime<Local>,
    #[derivative(Default(
        value = "\"rustic \".to_string() + option_env!(\"PROJECT_VERSION\").unwrap_or(env!(\"CARGO_PKG_VERSION\"))"
    ))]
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub program_version: String,
    pub parent: Option<Id>,
    pub tree: Id,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    pub paths: StringList,
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub uid: u32,
    #[serde(default)]
    pub gid: u32,
    #[serde(default)]
    pub tags: StringList,
    pub original: Option<Id>,
    #[serde(default, skip_serializing_if = "DeleteOption::is_not_set")]
    pub delete: DeleteOption,

    pub summary: Option<SnapshotSummary>,
    pub description: Option<String>,

    #[serde(default, skip_serializing_if = "Id::is_null")]
    pub id: Id,
}

impl RepoFile for SnapshotFile {
    const TYPE: FileType = FileType::Snapshot;
}

impl SnapshotFile {
    pub fn new_from_options(
        opts: SnapshotOptions,
        time: DateTime<Local>,
        command: String,
    ) -> Result<Self> {
        let hostname = match opts.host {
            Some(host) => host,
            None => {
                let hostname = gethostname();
                hostname
                    .to_str()
                    .ok_or_else(|| anyhow!("non-unicode hostname {:?}", hostname))?
                    .to_string()
            }
        };

        let delete = match (opts.delete_never, opts.delete_after) {
            (true, _) => DeleteOption::Never,
            (_, Some(d)) => DeleteOption::After(time + Duration::from_std(*d)?),
            (false, None) => DeleteOption::NotSet,
        };

        let mut snap = SnapshotFile {
            time,
            hostname,
            label: opts.label.unwrap_or_default(),
            delete,
            summary: Some(SnapshotSummary {
                command,
                ..Default::default()
            }),
            description: opts.description,
            ..Default::default()
        };

        // use description from description file if it is given
        if let Some(file) = opts.description_from {
            snap.description = Some(std::fs::read_to_string(file)?);
        }

        snap.set_tags(opts.tag.clone());

        Ok(snap)
    }

    fn set_id(tuple: (Id, Self)) -> Self {
        let (id, mut snap) = tuple;
        snap.id = id;
        snap.original.get_or_insert(id);
        snap
    }

    /// Get a [`SnapshotFile`] from the backend
    pub fn from_backend<B: DecryptReadBackend>(be: &B, id: &Id) -> Result<Self> {
        Ok(Self::set_id((*id, be.get_file(id)?)))
    }

    pub fn from_str<B: DecryptReadBackend>(
        be: &B,
        string: &str,
        predicate: impl FnMut(&Self) -> bool + Send + Sync,
        p: ProgressBar,
    ) -> Result<Self> {
        match string {
            "latest" => Self::latest(be, predicate, p),
            _ => Self::from_id(be, string),
        }
    }

    /// Get the latest [`SnapshotFile`] from the backend
    pub fn latest<B: DecryptReadBackend>(
        be: &B,
        predicate: impl FnMut(&Self) -> bool + Send + Sync,
        p: ProgressBar,
    ) -> Result<Self> {
        p.set_prefix("getting latest snapshot...");
        let mut latest: Option<Self> = None;
        let mut pred = predicate;

        for snap in be.stream_all::<SnapshotFile>(p.clone())? {
            let (id, mut snap) = snap?;
            if !pred(&snap) {
                continue;
            }

            snap.id = id;
            match &latest {
                Some(l) if l.time > snap.time => {}
                _ => {
                    latest = Some(snap);
                }
            }
        }
        p.finish();
        latest.ok_or_else(|| anyhow!("no snapshots found"))
    }

    /// Get a [`SnapshotFile`] from the backend by (part of the) id
    pub fn from_id<B: DecryptReadBackend>(be: &B, id: &str) -> Result<Self> {
        info!("getting snapshot...");
        let id = be.find_id(FileType::Snapshot, id)?;
        SnapshotFile::from_backend(be, &id)
    }

    /// Get a Vector of [`SnapshotFile`] from the backend by list of (parts of the) ids
    pub fn from_ids<B: DecryptReadBackend>(be: &B, ids: &[String]) -> Result<Vec<Self>> {
        let ids = be.find_ids(FileType::Snapshot, ids)?;
        be.stream_list::<Self>(ids, ProgressBar::hidden())?
            .into_iter()
            .map_ok(Self::set_id)
            .try_collect()
    }

    fn cmp_group(&self, crit: &SnapshotGroupCriterion, other: &Self) -> Ordering {
        match crit.hostname {
            false => Ordering::Equal,
            true => self.hostname.cmp(&other.hostname),
        }
        .then_with(|| match crit.label {
            false => Ordering::Equal,
            true => self.label.cmp(&other.label),
        })
        .then_with(|| match crit.paths {
            false => Ordering::Equal,
            true => self.paths.cmp(&other.paths),
        })
        .then_with(|| match crit.tags {
            false => Ordering::Equal,
            true => self.tags.cmp(&other.tags),
        })
    }

    pub fn has_group(&self, group: &SnapshotGroup) -> bool {
        (match &group.hostname {
            Some(val) => val == &self.hostname,
            None => true,
        }) && (match &group.label {
            Some(val) => val == &self.label,
            None => true,
        }) && (match &group.paths {
            Some(val) => val == &self.paths,
            None => true,
        }) && (match &group.tags {
            Some(val) => val == &self.tags,
            None => true,
        })
    }

    /// Get [`SnapshotFile`]s which match the filter grouped by the group criterion
    /// from the backend
    pub fn group_from_backend<B: DecryptReadBackend>(
        be: &B,
        filter: &SnapshotFilter,
        crit: &SnapshotGroupCriterion,
    ) -> Result<Vec<(SnapshotGroup, Vec<Self>)>> {
        let mut snaps = Self::all_from_backend(be, filter)?;
        snaps.sort_unstable_by(|sn1, sn2| sn1.cmp_group(crit, sn2));

        let mut result = Vec::new();
        for (group, snaps) in &snaps
            .into_iter()
            .group_by(|sn| SnapshotGroup::from_sn(sn, crit))
        {
            result.push((group, snaps.collect()));
        }

        Ok(result)
    }

    pub fn all_from_backend<B: DecryptReadBackend>(
        be: &B,
        filter: &SnapshotFilter,
    ) -> Result<Vec<Self>> {
        be.stream_all::<SnapshotFile>(ProgressBar::hidden())?
            .into_iter()
            .map_ok(Self::set_id)
            .filter_ok(|sn| sn.matches(filter))
            .try_collect()
    }

    pub fn matches(&self, filter: &SnapshotFilter) -> bool {
        if let Some(filter_fn) = &filter.filter_fn {
            match filter_fn.call::<bool>(self) {
                Ok(result) => {
                    if !result {
                        return false;
                    }
                }
                Err(err) => {
                    warn!("Error evaluating filter-fn for snapshot {}: {err}", self.id);
                }
            }
        }

        self.paths.matches(&filter.filter_paths)
            && self.tags.matches(&filter.filter_tags)
            && (filter.filter_host.is_empty() || filter.filter_host.contains(&self.hostname))
            && (filter.filter_label.is_empty() || filter.filter_label.contains(&self.label))
    }

    /// Add tag lists to snapshot. return whether snapshot was changed
    pub fn add_tags(&mut self, tag_lists: Vec<StringList>) -> bool {
        let old_tags = self.tags.clone();
        self.tags.add_all(tag_lists);
        self.tags.sort();

        old_tags != self.tags
    }

    /// Set tag lists to snapshot. return whether snapshot was changed
    pub fn set_tags(&mut self, tag_lists: Vec<StringList>) -> bool {
        let old_tags = std::mem::take(&mut self.tags);
        self.tags.add_all(tag_lists);
        self.tags.sort();

        old_tags != self.tags
    }

    /// Remove tag lists from snapshot. return whether snapshot was changed
    pub fn remove_tags(&mut self, tag_lists: Vec<StringList>) -> bool {
        let old_tags = self.tags.clone();
        self.tags.remove_all(tag_lists);

        old_tags != self.tags
    }

    /// Returns whether a snapshot must be deleted now
    pub fn must_delete(&self, now: DateTime<Local>) -> bool {
        matches!(self.delete,DeleteOption::After(time) if time < now)
    }

    /// Returns whether a snapshot must be kept now
    pub fn must_keep(&self, now: DateTime<Local>) -> bool {
        match self.delete {
            DeleteOption::Never => true,
            DeleteOption::After(time) if time >= now => true,
            _ => false,
        }
    }
}

impl PartialEq<SnapshotFile> for SnapshotFile {
    fn eq(&self, other: &SnapshotFile) -> bool {
        self.time.eq(&other.time)
    }
}
impl Eq for SnapshotFile {}

impl PartialOrd for SnapshotFile {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.time.partial_cmp(&other.time)
    }
}
impl Ord for SnapshotFile {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time.cmp(&other.time)
    }
}

#[derive(Clone, Debug)]
struct SnapshotFn(FnPtr, AST);
impl FromStr for SnapshotFn {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        let engine = Engine::new();
        let ast = engine.compile(s)?;
        let func = engine.eval_ast::<FnPtr>(&ast)?;
        Ok(Self(func, ast))
    }
}

impl SnapshotFn {
    fn call<T: Clone + Send + Sync + 'static>(&self, sn: &SnapshotFile) -> Result<T> {
        let engine = Engine::new();
        let sn: Dynamic = to_dynamic(sn)?;
        Ok(self.0.call::<T>(&engine, &self.1, (sn,))?)
    }
}

#[serde_as]
#[derive(Clone, Default, Debug, Parser, Deserialize, Merge)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct SnapshotFilter {
    /// Hostname to filter (can be specified multiple times)
    #[clap(long, global = true, value_name = "HOSTNAME")]
    #[merge(strategy=merge::vec::overwrite_empty)]
    filter_host: Vec<String>,

    /// Label to filter (can be specified multiple times)
    #[clap(long, global = true, value_name = "LABEL")]
    #[merge(strategy=merge::vec::overwrite_empty)]
    filter_label: Vec<String>,

    /// Path list to filter (can be specified multiple times)
    #[clap(long, global = true, value_name = "PATH[,PATH,..]")]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[merge(strategy=merge::vec::overwrite_empty)]
    filter_paths: Vec<StringList>,

    /// Tag list to filter (can be specified multiple times)
    #[clap(long, global = true, value_name = "TAG[,TAG,..]")]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[merge(strategy=merge::vec::overwrite_empty)]
    filter_tags: Vec<StringList>,

    /// Function to filter snapshots
    #[clap(long, global = true, value_name = "FUNC")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    filter_fn: Option<SnapshotFn>,
}

#[derive(Clone, Default, Debug, DeserializeFromStr)]
pub struct SnapshotGroupCriterion {
    hostname: bool,
    label: bool,
    paths: bool,
    tags: bool,
}

impl FromStr for SnapshotGroupCriterion {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        let mut crit = SnapshotGroupCriterion::default();
        for val in s.split(',') {
            match val {
                "host" => crit.hostname = true,
                "label" => crit.label = true,
                "paths" => crit.paths = true,
                "tags" => crit.tags = true,
                "" => continue,
                v => bail!("{} not allowed", v),
            }
        }
        Ok(crit)
    }
}

#[serde_with::apply(Option => #[serde(default, skip_serializing_if = "Option::is_none")])]
#[derive(Default, Debug, PartialEq, Eq, Serialize)]
pub struct SnapshotGroup {
    hostname: Option<String>,
    label: Option<String>,
    paths: Option<StringList>,
    tags: Option<StringList>,
}

impl Display for SnapshotGroup {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut out = Vec::new();

        if let Some(host) = &self.hostname {
            out.push(format!("host [{host}]"));
        }
        if let Some(label) = &self.label {
            out.push(format!("label [{label}]"));
        }
        if let Some(paths) = &self.paths {
            out.push(format!("paths [{paths}]"));
        }
        if let Some(tags) = &self.tags {
            out.push(format!("tags [{tags}]"));
        }

        write!(f, "({})", out.join(", "))?;
        Ok(())
    }
}

impl SnapshotGroup {
    pub fn from_sn(sn: &SnapshotFile, crit: &SnapshotGroupCriterion) -> Self {
        Self {
            hostname: crit.hostname.then(|| sn.hostname.clone()),
            label: crit.label.then(|| sn.label.clone()),
            paths: crit.paths.then(|| sn.paths.clone()),
            tags: crit.tags.then(|| sn.tags.clone()),
        }
    }

    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

#[derive(Default, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Serialize, Deserialize)]
pub struct StringList(Vec<String>);

impl FromStr for StringList {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        Ok(StringList(s.split(',').map(|s| s.to_string()).collect()))
    }
}

impl Display for StringList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0.join(","))?;
        Ok(())
    }
}

impl StringList {
    pub fn contains(&self, s: &String) -> bool {
        self.0.contains(s)
    }

    pub fn contains_all(&self, sl: &StringList) -> bool {
        sl.0.iter().all(|s| self.contains(s))
    }

    pub fn matches(&self, sls: &[StringList]) -> bool {
        sls.is_empty() || sls.iter().any(|sl| self.contains_all(sl))
    }

    pub fn add(&mut self, s: String) {
        if !self.contains(&s) {
            self.0.push(s);
        }
    }

    pub fn add_list(&mut self, sl: StringList) {
        for s in sl.0 {
            self.add(s);
        }
    }

    pub fn add_all(&mut self, string_lists: Vec<StringList>) {
        for sl in string_lists {
            self.add_list(sl);
        }
    }

    pub fn set_paths(&mut self, paths: &[PathBuf]) -> Result<()> {
        self.0 = paths
            .iter()
            .map(|p| {
                Ok(p.to_str()
                    .ok_or_else(|| anyhow!("non-unicode path {:?}", p))?
                    .to_string())
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(())
    }

    pub fn remove_all(&mut self, string_lists: Vec<StringList>) {
        self.0
            .retain(|s| !string_lists.iter().any(|sl| sl.contains(s)));
    }

    pub fn sort(&mut self) {
        self.0.sort_unstable();
    }

    pub fn formatln(&self) -> String {
        self.0.join("\n")
    }

    pub fn iter(&self) -> std::slice::Iter<String> {
        self.0.iter()
    }
}

#[derive(Default, Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct PathList(Vec<PathBuf>);

impl Display for PathList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if !self.0.is_empty() {
            write!(f, "{:?}", self.0[0])?;
        }
        for p in &self.0[1..] {
            write!(f, ",{:?}", p)?;
        }
        Ok(())
    }
}

impl PathList {
    pub fn from_strings<I>(source: I, sanitize: bool) -> Result<Self>
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        let mut paths = PathList(
            source
                .into_iter()
                .map(|source| PathBuf::from(source.as_ref()))
                .collect(),
        );

        if sanitize {
            paths.sanitize()?;
        }
        paths.merge_paths();
        Ok(paths)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn from_string(sources: &str, sanitize: bool) -> Result<Self> {
        let sources = parse_command::<()>(sources)?.1;
        Self::from_strings(sources, sanitize)
    }

    pub fn paths(&self) -> Vec<PathBuf> {
        self.0.clone()
    }

    // sanitize paths: parse dots and absolutize if needed
    fn sanitize(&mut self) -> Result<()> {
        for path in &mut self.0 {
            *path = path.parse_dot()?.to_path_buf();
        }
        if self.0.iter().any(|p| p.is_absolute()) {
            for path in &mut self.0 {
                *path = canonicalize(&path)?;
            }
        }
        Ok(())
    }

    // sort paths and filters out subpaths of already existing paths
    fn merge_paths(&mut self) {
        // sort paths
        self.0.sort_unstable();

        let mut root_path = None;

        // filter out subpaths
        self.0.retain(|path| match &root_path {
            Some(root_path) if path.starts_with(root_path) => false,
            _ => {
                root_path = Some(path.to_path_buf());
                true
            }
        });
    }
}
