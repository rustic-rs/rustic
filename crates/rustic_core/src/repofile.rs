use serde::{de::DeserializeOwned, Serialize};

pub(crate) mod configfile;
pub(crate) mod indexfile;
pub(crate) mod keyfile;
pub(crate) mod packfile;
pub(crate) mod snapshotfile;

/// Marker trait for repository files which are stored as encrypted JSON
pub trait RepoFile: Serialize + DeserializeOwned + Sized + Send + Sync + 'static {
    /// The [`FileType`] associated with the repository file
    const TYPE: FileType;
}

// Part of public API

pub use {
    crate::{
        backend::{
            node::{Node, NodeType},
            FileType, ALL_FILE_TYPES,
        },
        blob::{tree::Tree, BlobType, ALL_BLOB_TYPES},
    },
    configfile::ConfigFile,
    indexfile::{IndexBlob, IndexFile, IndexPack},
    keyfile::KeyFile,
    packfile::{HeaderEntry, PackHeader, PackHeaderLength, PackHeaderRef},
    snapshotfile::{DeleteOption, PathList, SnapshotFile, SnapshotSummary, StringList},
};
