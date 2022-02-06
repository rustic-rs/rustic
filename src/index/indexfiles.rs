use super::{IndexEntry, ReadIndex};
use crate::backend::{FileType, ReadBackend};
use crate::id::Id;
use crate::repo::IndexFile;

#[derive(Clone)]
pub struct AllIndexFiles<BE> {
    be: BE,
}

impl<BE: ReadBackend> AllIndexFiles<BE> {
    pub fn new(be: BE) -> Self {
        Self { be: be }
    }
}

impl<BE: ReadBackend> AllIndexFiles<BE> {
    pub fn into_iter(self) -> impl Iterator<Item = IndexEntry> {
        self.be
            .list(FileType::Index)
            .unwrap()
            .into_iter()
            .flat_map(move |id| {
                IndexFile::from_backend(&self.be, id)
                    .unwrap()
                    .packs()
                    .into_iter()
                    .flat_map(|p| {
                        let id = p.id().clone();
                        p.blobs()
                            .into_iter()
                            .map(move |b| IndexEntry::new(id, b.to_bi()))
                    })
            })
    }
}

impl<BE: ReadBackend> ReadIndex for AllIndexFiles<BE> {
    fn get_id(&self, id: &Id) -> Option<IndexEntry> {
        self.clone().into_iter().find(|ie| ie.id() == id)
    }
}
