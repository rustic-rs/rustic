use std::cmp::Ordering;
use std::str::FromStr;

use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, Local};
use clap::Parser;
use futures::{future, TryStreamExt};
use indicatif::ProgressBar;
use serde::{Deserialize, Serialize};
use vlog::*;

use super::Id;
use crate::backend::{DecryptReadBackend, FileType, RepoFile};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotFile {
    pub time: DateTime<Local>,
    pub tree: Id,
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

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup_start: Option<DateTime<Local>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup_end: Option<DateTime<Local>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files_new: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files_changed: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files_unchanged: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trees_new: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trees_changed: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trees_unchanged: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_blobs_written: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree_blobs_written: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_added: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,

    #[serde(skip)]
    pub id: Id,
}

impl RepoFile for SnapshotFile {
    const TYPE: FileType = FileType::Snapshot;
}

impl Default for SnapshotFile {
    fn default() -> Self {
        Self {
            time: Local::now(),
            tree: Id::default(),
            paths: StringList::default(),
            hostname: String::default(),
            username: String::default(),
            uid: 0,
            gid: 0,
            tags: StringList::default(),
            node_count: None,
            size: None,
            command: None,
            backup_start: None,
            backup_end: None,
            files_new: Some(0),
            files_changed: Some(0),
            files_unchanged: Some(0),
            trees_new: Some(0),
            trees_changed: Some(0),
            trees_unchanged: Some(0),
            data_blobs_written: Some(0),
            tree_blobs_written: Some(0),
            data_added: Some(0),

            id: Id::default(),
        }
    }
}

#[derive(Parser)]
pub struct SnapshotFilter {
    /// Path list to filter (can be specified multiple times)
    #[clap(long = "filter-paths")]
    paths: Vec<StringList>,

    /// Tag list to filter (can be specified multiple times)
    #[clap(long = "filter-tags")]
    tags: Vec<StringList>,

    /// Hostname to filter (can be specified multiple times)
    #[clap(long = "filter-host", value_name = "HOSTNAME")]
    hostnames: Vec<String>,
}

#[derive(Default)]
pub struct SnapshotGroupCriterion {
    hostname: bool,
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
                "paths" => crit.paths = true,
                "tags" => crit.tags = true,
                "" => continue,
                v => bail!("{} not allowed", v),
            }
        }
        Ok(crit)
    }
}

#[derive(Default, Debug)]
pub struct SnapshotGroup {
    hostname: Option<String>,
    paths: Option<StringList>,
    tags: Option<StringList>,
}

impl SnapshotGroup {
    pub fn from_sn(sn: &SnapshotFile, crit: &SnapshotGroupCriterion) -> Self {
        Self {
            hostname: crit.hostname.then(|| sn.hostname.clone()),
            paths: crit.paths.then(|| sn.paths.clone()),
            tags: crit.tags.then(|| sn.tags.clone()),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.hostname.is_none() && self.paths.is_none() && self.tags.is_none()
    }
}

impl SnapshotFile {
    /// Get a SnapshotFile from the backend
    pub async fn from_backend<B: DecryptReadBackend>(be: &B, id: &Id) -> Result<Self> {
        let mut snap: Self = be.get_file(id).await?;
        snap.id = *id;
        Ok(snap)
    }

    pub async fn from_str<B: DecryptReadBackend>(
        be: &B,
        string: &str,
        predicate: impl FnMut(&Self) -> bool,
        p: ProgressBar,
    ) -> Result<Self> {
        match string {
            "latest" => Self::latest(be, predicate, p).await,
            _ => Self::from_id(be, string).await,
        }
    }

    /// Get the latest SnapshotFile from the backend
    pub async fn latest<B: DecryptReadBackend>(
        be: &B,
        predicate: impl FnMut(&Self) -> bool,
        p: ProgressBar,
    ) -> Result<Self> {
        v1!("getting latest snapshot...");
        let mut latest: Option<Self> = None;
        let mut pred = predicate;
        let mut snaps = be.stream_all::<SnapshotFile>(p.clone()).await?;

        while let Some((id, mut snap)) = snaps.try_next().await? {
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
        p.finish_with_message("done.");
        latest.ok_or_else(|| anyhow!("no snapshots found"))
    }

    /// Get a SnapshotFile from the backend by (part of the) id
    pub async fn from_id<B: DecryptReadBackend>(be: &B, id: &str) -> Result<Self> {
        v1!("getting snapshot...");
        let id = be.find_id(FileType::Snapshot, id).await?;
        SnapshotFile::from_backend(be, &id).await
    }

    fn cmp_group(&self, crit: &SnapshotGroupCriterion, other: &Self) -> Ordering {
        match crit.hostname {
            false => Ordering::Equal,
            true => self.hostname.cmp(&other.hostname),
        }
        .then_with(|| match crit.paths {
            false => Ordering::Equal,
            true => self.paths.cmp(&other.paths),
        })
        .then_with(|| match crit.tags {
            false => Ordering::Equal,
            true => self.tags.cmp(&other.tags),
        })
    }

    fn has_group(&self, group: &SnapshotGroup) -> bool {
        (match &group.hostname {
            Some(val) => val == &self.hostname,
            None => true,
        }) && (match &group.paths {
            Some(val) => val == &self.paths,
            None => true,
        }) && (match &group.tags {
            Some(val) => val == &self.tags,
            None => true,
        })
    }

    /// Get SnapshotFiles which match the filter grouped by the group criterion
    /// from the backend
    pub async fn group_from_backend<B: DecryptReadBackend>(
        be: &B,
        filter: &SnapshotFilter,
        crit: &SnapshotGroupCriterion,
    ) -> Result<Vec<(SnapshotGroup, Vec<Self>)>> {
        let mut snaps = Self::all_from_backend(be, filter).await?;
        snaps.sort_unstable_by(|sn1, sn2| sn1.cmp_group(crit, sn2));

        let mut result = Vec::new();

        if snaps.is_empty() {
            return Ok(result);
        }

        let mut iter = snaps.into_iter();

        let snap = iter.next().unwrap();
        let mut group = SnapshotGroup::from_sn(&snap, crit);
        let mut result_group = vec![snap];

        for snap in iter {
            if snap.has_group(&group) {
                result_group.push(snap);
            } else {
                result.push((group, result_group));
                group = SnapshotGroup::from_sn(&snap, crit);
                result_group = vec![snap]
            }
        }
        result.push((group, result_group));

        Ok(result)
    }

    pub async fn all_from_backend<B: DecryptReadBackend>(
        be: &B,
        filter: &SnapshotFilter,
    ) -> Result<Vec<Self>> {
        Ok(be
            .stream_all::<SnapshotFile>(ProgressBar::hidden())
            .await?
            .map_ok(|(id, mut snap)| {
                snap.id = id;
                snap
            })
            .try_filter(|sn| future::ready(sn.matches(filter)))
            .try_collect()
            .await?)
    }

    pub fn matches(&self, filter: &SnapshotFilter) -> bool {
        self.paths.matches(&filter.paths)
            && self.tags.matches(&filter.tags)
            && (filter.hostnames.is_empty() || filter.hostnames.contains(&self.hostname))
    }

    /// Add tag lists to snapshot. return wheter snapshot was changed
    pub fn add_tags(&mut self, tag_lists: Vec<StringList>) -> bool {
        let old_tags = self.tags.clone();
        self.tags.add_all(tag_lists);
        self.tags.sort();

        old_tags == self.tags
    }

    /// Set tag lists to snapshot. return wheter snapshot was changed
    pub fn set_tags(&mut self, tag_lists: Vec<StringList>) -> bool {
        let old_tags = std::mem::take(&mut self.tags);
        self.tags.add_all(tag_lists);
        self.tags.sort();

        old_tags == self.tags
    }

    /// Remove tag lists from snapshot. return wheter snapshot was changed
    pub fn remove_tags(&mut self, tag_lists: Vec<StringList>) -> bool {
        let old_tags = self.tags.clone();
        self.tags.remove_all(tag_lists);

        old_tags == self.tags
    }
}

impl PartialOrd for SnapshotFile {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.time.partial_cmp(&other.time)
    }
}
impl Ord for SnapshotFile {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.time.cmp(&other.time)
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
            self.add(s)
        }
    }

    pub fn add_all(&mut self, string_lists: Vec<StringList>) {
        for sl in string_lists {
            self.add_list(sl)
        }
    }

    pub fn remove_all(&mut self, string_lists: Vec<StringList>) {
        self.0
            .retain(|s| !string_lists.iter().any(|sl| sl.contains(s)));
    }

    pub fn sort(&mut self) {
        self.0.sort_unstable();
    }

    pub fn formatln(&self) -> String {
        self.0
            .iter()
            .map(|p| p.to_string() + "\n")
            .collect::<String>()
    }
}
