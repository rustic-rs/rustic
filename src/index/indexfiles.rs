use anyhow::Result;
use vlog::*;

use crate::backend::{DecryptReadBackend, FileType};
use crate::repo::{IndexFile, IndexPack};

#[derive(Clone)]
pub struct AllIndexFiles<BE> {
    be: BE,
}

impl<BE: DecryptReadBackend> AllIndexFiles<BE> {
    pub fn new(be: BE) -> Self {
        Self { be }
    }
}

impl<BE: DecryptReadBackend> AllIndexFiles<BE> {
    pub fn into_iter(self) -> Result<impl Iterator<Item = IndexPack>> {
        v1!("reading index...");

        Ok(self
            .be
            .list(FileType::Index)?
            .into_iter()
            .flat_map(move |id| {
                let (_, packs) = IndexFile::from_backend(&self.be, &id).unwrap().dissolve();
                packs
            }))
    }
}
