use anyhow::Result;

use crate::backend::{FileType, ReadBackend};
use crate::repo::{IndexFile, IndexPack};

#[derive(Clone)]
pub struct AllIndexFiles<BE> {
    be: BE,
}

impl<BE: ReadBackend> AllIndexFiles<BE> {
    pub fn new(be: BE) -> Self {
        Self { be }
    }
}

impl<BE: ReadBackend> AllIndexFiles<BE> {
    pub fn into_iter(self) -> Result<impl Iterator<Item = IndexPack>> {
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
