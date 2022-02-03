use super::{IndexEntry, ReadIndex};
use crate::backend::{FileType, ReadBackend};
use crate::id::Id;
use crate::repo::IndexFile;

pub struct AllIndexFiles<'a, BE> {
    be: &'a BE,
    files: Vec<Id>,
}

impl<'a, BE: ReadBackend> AllIndexFiles<'a, BE> {
    pub fn new(be: &'a BE) -> Self {
        Self {
            be: be,
            files: be.list(FileType::Index).unwrap(),
        }
    }
}

impl<'a, BE: ReadBackend> ReadIndex for AllIndexFiles<'a, BE> {
    fn iter(&self) -> Box<dyn Iterator<Item = IndexEntry> + '_> {
        Box::new(
            self.files
                .iter()
                .flat_map(|id| IndexFile::from_backend(self.be, *id).unwrap().into_iter()),
        )
    }
}
