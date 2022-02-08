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
    pub fn into_iter(self) -> impl Iterator<Item = IndexPack> {
        self.be
            .list(FileType::Index)
            .unwrap()
            .into_iter()
            .flat_map(move |id| {
                let (_, packs) = IndexFile::from_backend(&self.be, id).unwrap().dissolve();
                packs
            })
    }
}
