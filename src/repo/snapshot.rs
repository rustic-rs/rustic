use anyhow::{anyhow, Result};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use super::Id;
use crate::backend::{FileType, ReadBackend, WriteBackend};

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

impl SnapshotFile {
    pub fn new(
        tree: Id,
        paths: Vec<String>,
        hostname: String,
        username: String,
        uid: u32,
        gid: u32,
        tags: TagList,
        node_count: Option<u64>,
        size: Option<u64>,
    ) -> Self {
        Self {
            time: Local::now(),
            tree,
            paths,
            hostname,
            username,
            uid,
            gid,
            tags,
            node_count,
            size,
            id: Id::default(),
        }
    }

    /// Get a SnapshotFile from the backend
    pub fn from_backend<B: ReadBackend>(be: &B, id: &Id) -> Result<Self> {
        let data = be.read_full(FileType::Snapshot, id)?;
        let mut snap: Self = serde_json::from_slice(&data)?;
        snap.set_id(*id);
        Ok(snap)
    }

    pub fn from_str<B: ReadBackend>(
        be: &B,
        string: &str,
        predicate: impl FnMut(&Self) -> bool,
    ) -> Result<Self> {
        match string {
            "latest" => Self::latest(be, predicate),
            _ => Self::from_id(be, string),
        }
    }

    /// Get the latest SnapshotFile from the backend
    pub fn latest<B: ReadBackend>(be: &B, predicate: impl FnMut(&Self) -> bool) -> Result<Self> {
        let mut latest: Option<Self> = None;
        let mut pred = predicate;
        for snap in be
            .list(FileType::Snapshot)?
            .iter()
            .map(|id| SnapshotFile::from_backend(be, &id))
        {
            let snap = snap?;
            if !pred(&snap) {
                continue;
            }
            match &latest {
                Some(l) if l.time > snap.time => {}
                _ => {
                    latest = Some(snap);
                }
            }
        }
        latest.ok_or_else(|| anyhow!("no snapshots found"))
    }

    /// Get a SnapshotFile from the backend by (part of the) id
    pub fn from_id<B: ReadBackend>(be: &B, id: &str) -> Result<Self> {
        let id = Id::from_hex(id).or_else(|_| {
            // if the given id param is not a full Id, search for a suitable one
            be.find_starts_with(FileType::Snapshot, &[id])?.remove(0)
        })?;
        SnapshotFile::from_backend(be, &id)
    }

    /// Get all SnapshotFiles from the backend
    pub fn all_from_backend<B: ReadBackend>(be: &B) -> Result<Vec<Self>> {
        let snapshots: Vec<_> = be
            .list(FileType::Snapshot)?
            .iter()
            .map(|id| SnapshotFile::from_backend(be, id))
            .collect::<Result<_, _>>()?;
        Ok(snapshots)
    }

    /// Save a SnapshotFile to the backend
    pub fn save_to_backend<B: WriteBackend>(&self, be: &B) -> Result<Id> {
        let data = serde_json::to_vec(&self)?;
        Ok(be.hash_write_full(FileType::Snapshot, &data)?)
    }

    pub fn set_id(&mut self, id: Id) {
        self.id = id;
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
