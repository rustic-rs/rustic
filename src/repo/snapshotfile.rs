use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Local};
use futures::TryStreamExt;
use indicatif::ProgressBar;
use serde::{Deserialize, Serialize};
use vlog::*;

use super::Id;
use crate::backend::{DecryptReadBackend, FileType, RepoFile};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotFile {
    pub time: DateTime<Local>,
    pub tree: Id,
    pub paths: Vec<String>,
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub uid: u32,
    #[serde(default)]
    pub gid: u32,
    #[serde(default)]
    pub tags: TagList,
    pub node_count: Option<u64>,
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
            paths: Vec::new(),
            hostname: String::default(),
            username: String::default(),
            uid: 0,
            gid: 0,
            tags: TagList::default(),
            node_count: None,
            size: None,
            id: Id::default(),
        }
    }
}

impl SnapshotFile {
    /// Get a SnapshotFile from the backend
    pub async fn from_backend<B: DecryptReadBackend>(be: &B, id: &Id) -> Result<Self> {
        let mut snap: Self = be.get_file(id).await?;
        snap.set_id(*id);
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
        let mut snaps = be.stream_all::<SnapshotFile>(p.clone())?;

        while let Some((id, mut snap)) = snaps.try_next().await? {
            if !pred(&snap) {
                continue;
            }
            snap.set_id(id);
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
        let id = Id::from_hex(id).or_else(|_| {
            // if the given id param is not a full Id, search for a suitable one
            be.find_starts_with(FileType::Snapshot, &[id])?.remove(0)
        })?;
        SnapshotFile::from_backend(be, &id).await
    }

    /// Get all SnapshotFiles from the backend
    pub async fn all_from_backend<B: DecryptReadBackend>(be: &B) -> Result<Vec<Self>> {
        Ok(be
            .stream_all::<SnapshotFile>(ProgressBar::hidden())?
            .map_ok(|(id, mut snap)| {
                snap.set_id(id);
                snap
            })
            .try_collect()
            .await?)
    }

    pub fn set_id(&mut self, id: Id) {
        self.id = id;
    }

    pub fn set_tree(&mut self, id: Id) {
        self.tree = id;
    }

    pub fn set_size(&mut self, size: u64) {
        self.size = Some(size);
    }

    pub fn set_hostname(&mut self, name: OsString) {
        self.hostname = name.to_str().unwrap().to_string();
    }

    pub fn set_paths(&mut self, paths: Vec<PathBuf>) {
        self.paths = paths
            .into_iter()
            .map(|path| path.to_str().expect("non-unicode path {:?}").to_string())
            .collect();
    }

    pub fn set_count(&mut self, count: u64) {
        self.node_count = Some(count);
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

#[derive(Default, Debug, PartialEq, Eq, PartialOrd, Serialize, Deserialize)]
pub struct TagList(Vec<Tag>);

#[derive(Default, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Tag(String);
