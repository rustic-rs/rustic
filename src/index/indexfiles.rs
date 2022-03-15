use anyhow::Result;
use indicatif::ProgressBar;

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
    pub fn into_iter(self, p: ProgressBar) -> Result<impl Iterator<Item = IndexPack>> {
        let list = self.be.list(FileType::Index)?;
        p.set_length(list.len() as u64);

        Ok(list
            .into_iter()
            .inspect(move |_| p.inc(1))
            .flat_map(move |id| {
                let (_, packs) = self.be.get_file::<IndexFile>(&id).unwrap().dissolve();
                packs
            }))
    }
}
