use std::{
    cmp::Ordering,
    fmt::{self, Display},
    path::PathBuf,
    str::FromStr,
};

use chrono::{DateTime, Duration, Local};
use derivative::Derivative;
use dunce::canonicalize;
use gethostname::gethostname;
use itertools::Itertools;
use log::info;

use path_dedot::ParseDot;

use serde::{Deserialize, Serialize};

use serde_with::{serde_as, DeserializeFromStr, DisplayFromStr};

use crate::{
    backend::{decrypt::DecryptReadBackend, FileType},
    error::SnapshotFileErrorKind,
    id::Id,
    repofile::RepoFile,
    repository::parse_command,
    Progress, RusticError, RusticResult,
};

#[serde_as]
#[cfg_attr(feature = "merge", derive(merge::Merge))]
#[cfg_attr(feature = "clap", derive(clap::Parser))]
#[derive(Deserialize, Clone, Default, Debug)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct SnapshotOptions {
    /// Label snapshot with given label
    #[cfg_attr(feature = "clap", clap(long, value_name = "LABEL"))]
    label: Option<String>,

    /// Tags to add to snapshot (can be specified multiple times)
    #[cfg_attr(feature = "clap", clap(long, value_name = "TAG[,TAG,..]"))]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[cfg_attr(feature = "merge", merge(strategy = merge::vec::overwrite_empty))]
    tag: Vec<StringList>,

    /// Add description to snapshot
    #[cfg_attr(feature = "clap", clap(long, value_name = "DESCRIPTION"))]
    description: Option<String>,

    /// Add description to snapshot from file
    #[cfg_attr(
        feature = "clap",
        clap(long, value_name = "FILE", conflicts_with = "description")
    )]
    description_from: Option<PathBuf>,

    /// Mark snapshot as uneraseable
    #[cfg_attr(feature = "clap", clap(long, conflicts_with = "delete_after"))]
    #[cfg_attr(feature = "merge", merge(strategy = merge::bool::overwrite_false))]
    delete_never: bool,

    /// Mark snapshot to be deleted after given duration (e.g. 10d)
    #[cfg_attr(feature = "clap", clap(long, value_name = "DURATION"))]
    #[serde_as(as = "Option<DisplayFromStr>")]
    delete_after: Option<humantime::Duration>,

    /// Set the host name manually
    #[cfg_attr(feature = "clap", clap(long, value_name = "NAME"))]
    host: Option<String>,
}

/// This is an extended version of the summaryOutput structure of restic in
/// restic/internal/ui/backup$/json.go
#[derive(Serialize, Deserialize, Debug, Clone, Derivative)]
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
    pub fn finalize(&mut self, snap_time: DateTime<Local>) -> RusticResult<()> {
        let end_time = Local::now();
        self.backup_duration = (end_time - self.backup_start)
            .to_std()
            .map_err(SnapshotFileErrorKind::OutOfRange)?
            .as_secs_f64();
        self.total_duration = (end_time - snap_time)
            .to_std()
            .map_err(SnapshotFileErrorKind::OutOfRange)?
            .as_secs_f64();
        self.backup_end = end_time;
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Derivative, Copy)]
#[derivative(Default)]
pub enum DeleteOption {
    #[derivative(Default)]
    NotSet,
    Never,
    After(DateTime<Local>),
}

impl DeleteOption {
    const fn is_not_set(&self) -> bool {
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
        opts: &SnapshotOptions,
        time: DateTime<Local>,
        command: String,
    ) -> RusticResult<Self> {
        let hostname = if let Some(ref host) = opts.host {
            host.clone()
        } else {
            let hostname = gethostname();
            hostname
                .to_str()
                .ok_or_else(|| SnapshotFileErrorKind::NonUnicodeHostname(hostname.clone()))?
                .to_string()
        };

        let delete = match (opts.delete_never, opts.delete_after) {
            (true, _) => DeleteOption::Never,
            (_, Some(d)) => DeleteOption::After(
                time + Duration::from_std(*d).map_err(SnapshotFileErrorKind::OutOfRange)?,
            ),
            (false, None) => DeleteOption::NotSet,
        };

        let mut snap = Self {
            time,
            hostname,
            label: opts.label.clone().unwrap_or_default(),
            delete,
            summary: Some(SnapshotSummary {
                command,
                ..Default::default()
            }),
            description: opts.description.clone(),
            ..Default::default()
        };

        // use description from description file if it is given
        if let Some(ref file) = opts.description_from {
            snap.description = Some(
                std::fs::read_to_string(file)
                    .map_err(SnapshotFileErrorKind::ReadingDescriptionFailed)?,
            );
        }

        _ = snap.set_tags(opts.tag.clone());

        Ok(snap)
    }

    fn set_id(tuple: (Id, Self)) -> Self {
        let (id, mut snap) = tuple;
        snap.id = id;
        _ = snap.original.get_or_insert(id);
        snap
    }

    /// Get a [`SnapshotFile`] from the backend
    fn from_backend<B: DecryptReadBackend>(be: &B, id: &Id) -> RusticResult<Self> {
        Ok(Self::set_id((*id, be.get_file(id)?)))
    }

    pub fn from_str<B: DecryptReadBackend>(
        be: &B,
        string: &str,
        predicate: impl FnMut(&Self) -> bool + Send + Sync,
        p: &impl Progress,
    ) -> RusticResult<Self> {
        match string {
            "latest" => Self::latest(be, predicate, p),
            _ => Self::from_id(be, string),
        }
    }

    /// Get the latest [`SnapshotFile`] from the backend
    pub fn latest<B: DecryptReadBackend>(
        be: &B,
        predicate: impl FnMut(&Self) -> bool + Send + Sync,
        p: &impl Progress,
    ) -> RusticResult<Self> {
        p.set_title("getting latest snapshot...");
        let mut latest: Option<Self> = None;
        let mut pred = predicate;

        for snap in be.stream_all::<Self>(p)? {
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
        latest.ok_or_else(|| SnapshotFileErrorKind::NoSnapshotsFound.into())
    }

    /// Get a [`SnapshotFile`] from the backend by (part of the) id
    pub fn from_id<B: DecryptReadBackend>(be: &B, id: &str) -> RusticResult<Self> {
        info!("getting snapshot...");
        let id = be.find_id(FileType::Snapshot, id)?;
        Self::from_backend(be, &id)
    }

    /// Get a Vector of [`SnapshotFile`] from the backend by list of (parts of the) ids
    pub fn from_ids<B: DecryptReadBackend>(
        be: &B,
        ids: &[String],
        p: &impl Progress,
    ) -> RusticResult<Vec<Self>> {
        let ids = be.find_ids(FileType::Snapshot, ids)?;
        be.stream_list::<Self>(ids, p)?
            .into_iter()
            .map_ok(Self::set_id)
            .try_collect()
    }

    fn cmp_group(&self, crit: SnapshotGroupCriterion, other: &Self) -> Ordering {
        if crit.hostname {
            self.hostname.cmp(&other.hostname)
        } else {
            Ordering::Equal
        }
        .then_with(|| {
            if crit.label {
                self.label.cmp(&other.label)
            } else {
                Ordering::Equal
            }
        })
        .then_with(|| {
            if crit.paths {
                self.paths.cmp(&other.paths)
            } else {
                Ordering::Equal
            }
        })
        .then_with(|| {
            if crit.tags {
                self.tags.cmp(&other.tags)
            } else {
                Ordering::Equal
            }
        })
    }

    #[must_use]
    pub fn has_group(&self, group: &SnapshotGroup) -> bool {
        group
            .hostname
            .as_ref()
            .map_or(true, |val| val == &self.hostname)
            && group.label.as_ref().map_or(true, |val| val == &self.label)
            && group.paths.as_ref().map_or(true, |val| val == &self.paths)
            && group.tags.as_ref().map_or(true, |val| val == &self.tags)
    }

    /// Get [`SnapshotFile`]s which match the filter grouped by the group criterion
    /// from the backend
    pub fn group_from_backend<B, F>(
        be: &B,
        filter: F,
        crit: SnapshotGroupCriterion,
        p: &impl Progress,
    ) -> RusticResult<Vec<(SnapshotGroup, Vec<Self>)>>
    where
        B: DecryptReadBackend,
        F: FnMut(&Self) -> bool,
    {
        let mut snaps = Self::all_from_backend(be, filter, p)?;
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

    pub fn all_from_backend<B, F>(be: &B, filter: F, p: &impl Progress) -> RusticResult<Vec<Self>>
    where
        B: DecryptReadBackend,
        F: FnMut(&Self) -> bool,
    {
        be.stream_all::<Self>(p)?
            .into_iter()
            .map_ok(Self::set_id)
            .filter_ok(filter)
            .try_collect()
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
    pub fn remove_tags(&mut self, tag_lists: &[StringList]) -> bool {
        let old_tags = self.tags.clone();
        self.tags.remove_all(tag_lists);

        old_tags != self.tags
    }

    /// Returns whether a snapshot must be deleted now
    #[must_use]
    pub fn must_delete(&self, now: DateTime<Local>) -> bool {
        matches!(self.delete,DeleteOption::After(time) if time < now)
    }

    /// Returns whether a snapshot must be kept now
    #[must_use]
    pub fn must_keep(&self, now: DateTime<Local>) -> bool {
        match self.delete {
            DeleteOption::Never => true,
            DeleteOption::After(time) if time >= now => true,
            _ => false,
        }
    }

    pub fn modify_sn(
        &mut self,
        set: Vec<StringList>,
        add: Vec<StringList>,
        remove: &[StringList],
        delete: &Option<DeleteOption>,
    ) -> Option<Self> {
        let mut changed = false;

        if !set.is_empty() {
            changed |= self.set_tags(set);
        }
        changed |= self.add_tags(add);
        changed |= self.remove_tags(remove);

        if let Some(delete) = delete {
            if &self.delete != delete {
                self.delete = *delete;
                changed = true;
            }
        }

        changed.then_some(self.clone())
    }

    // clear ids which are not saved by the copy command (and not compared when checking if snapshots already exist in the copy target)
    #[must_use]
    pub fn clear_ids(mut sn: Self) -> Self {
        sn.id = Id::default();
        sn.parent = None;
        sn
    }
}

impl PartialEq<Self> for SnapshotFile {
    fn eq(&self, other: &Self) -> bool {
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

#[allow(clippy::struct_excessive_bools)]
#[derive(DeserializeFromStr, Clone, Default, Debug, Copy)]
pub struct SnapshotGroupCriterion {
    hostname: bool,
    label: bool,
    paths: bool,
    tags: bool,
}

impl FromStr for SnapshotGroupCriterion {
    type Err = RusticError;
    fn from_str(s: &str) -> RusticResult<Self> {
        let mut crit = Self::default();
        for val in s.split(',') {
            match val {
                "host" => crit.hostname = true,
                "label" => crit.label = true,
                "paths" => crit.paths = true,
                "tags" => crit.tags = true,
                "" => continue,
                v => return Err(SnapshotFileErrorKind::ValueNotAllowed(v.into()).into()),
            }
        }
        Ok(crit)
    }
}

#[serde_with::apply(Option => #[serde(default, skip_serializing_if = "Option::is_none")])]
#[derive(Serialize, Default, Debug, PartialEq, Eq)]
pub struct SnapshotGroup {
    hostname: Option<String>,
    label: Option<String>,
    paths: Option<StringList>,
    tags: Option<StringList>,
}

impl Display for SnapshotGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    #[must_use]
    pub fn from_sn(sn: &SnapshotFile, crit: SnapshotGroupCriterion) -> Self {
        Self {
            hostname: crit.hostname.then(|| sn.hostname.clone()),
            label: crit.label.then(|| sn.label.clone()),
            paths: crit.paths.then(|| sn.paths.clone()),
            tags: crit.tags.then(|| sn.tags.clone()),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

#[derive(Serialize, Deserialize, Default, Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct StringList(Vec<String>);

impl FromStr for StringList {
    type Err = RusticError;
    fn from_str(s: &str) -> RusticResult<Self> {
        Ok(Self(
            s.split(',').map(std::string::ToString::to_string).collect(),
        ))
    }
}

impl Display for StringList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.join(","))?;
        Ok(())
    }
}

impl StringList {
    fn contains(&self, s: &String) -> bool {
        self.0.contains(s)
    }

    fn contains_all(&self, sl: &Self) -> bool {
        sl.0.iter().all(|s| self.contains(s))
    }

    #[must_use]
    pub fn matches(&self, sls: &[Self]) -> bool {
        sls.is_empty() || sls.iter().any(|sl| self.contains_all(sl))
    }

    fn add(&mut self, s: String) {
        if !self.contains(&s) {
            self.0.push(s);
        }
    }

    fn add_list(&mut self, sl: Self) {
        for s in sl.0 {
            self.add(s);
        }
    }

    fn add_all(&mut self, string_lists: Vec<Self>) {
        for sl in string_lists {
            self.add_list(sl);
        }
    }

    pub fn set_paths(&mut self, paths: &[PathBuf]) -> RusticResult<()> {
        self.0 = paths
            .iter()
            .map(|p| {
                Ok(p.to_str()
                    .ok_or_else(|| SnapshotFileErrorKind::NonUnicodePath(p.into()))?
                    .to_string())
            })
            .collect::<RusticResult<Vec<_>>>()?;
        Ok(())
    }

    fn remove_all(&mut self, string_lists: &[Self]) {
        self.0
            .retain(|s| !string_lists.iter().any(|sl| sl.contains(s)));
    }

    fn sort(&mut self) {
        self.0.sort_unstable();
    }

    #[must_use]
    pub fn formatln(&self) -> String {
        self.0.join("\n")
    }

    pub fn iter(&self) -> std::slice::Iter<'_, String> {
        self.0.iter()
    }
}

#[derive(Default, Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct PathList(Vec<PathBuf>);

impl Display for PathList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.0.is_empty() {
            write!(f, "{:?}", self.0[0])?;
        }
        for p in &self.0[1..] {
            write!(f, ",{p:?}")?;
        }
        Ok(())
    }
}

impl PathList {
    pub fn from_strings<I>(source: I, sanitize: bool) -> RusticResult<Self>
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        let mut paths = Self(
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

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.len() == 0
    }

    pub fn from_string(sources: &str, sanitize: bool) -> RusticResult<Self> {
        let sources = parse_command::<()>(sources)
            .map_err(SnapshotFileErrorKind::FromNomError)?
            .1;
        Self::from_strings(sources, sanitize)
    }

    #[must_use]
    pub fn paths(&self) -> Vec<PathBuf> {
        self.0.clone()
    }

    // sanitize paths: parse dots and absolutize if needed
    fn sanitize(&mut self) -> RusticResult<()> {
        for path in &mut self.0 {
            *path = path
                .parse_dot()
                .map_err(SnapshotFileErrorKind::RemovingDotsFromPathFailed)?
                .to_path_buf();
        }
        if self.0.iter().any(|p| p.is_absolute()) {
            for path in &mut self.0 {
                *path =
                    canonicalize(&path).map_err(SnapshotFileErrorKind::CanonicalizingPathFailed)?;
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
                root_path = Some(path.clone());
                true
            }
        });
    }
}
