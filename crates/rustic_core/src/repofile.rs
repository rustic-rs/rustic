use serde::{de::DeserializeOwned, Serialize};

use crate::FileType;

pub(crate) mod configfile;
pub(crate) mod indexfile;
pub(crate) mod keyfile;
pub(crate) mod packfile;
pub(crate) mod snapshotfile;

pub trait RepoFile: Serialize + DeserializeOwned + Sized + Send + Sync + 'static {
    const TYPE: FileType;
}
