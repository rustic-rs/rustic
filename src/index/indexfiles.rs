use super::{IndexEntry, ReadIndex};
use crate::backend::{FileType, ReadBackend};
use crate::id::Id;
use crate::repo::IndexFile;

pub struct AllIndexFiles<BE> {
    be: BE,
    files: Vec<Id>,
}

impl<BE: ReadBackend> AllIndexFiles<BE> {
    pub fn new(be: &BE) -> Self {
        Self {
            be: be.clone(),
            files: be.list(FileType::Index).unwrap(),
        }
    }
}

impl<BE: ReadBackend> ReadIndex for AllIndexFiles<BE> {
    fn iter(&self) -> Box<dyn Iterator<Item = IndexEntry> + '_> {
        Box::new(
            self.files
                .iter()
                .flat_map(|&id| IndexFile::from_backend(&self.be, id).unwrap().into_iter()),
        )
    }
}
