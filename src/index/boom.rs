use boomphf::hashmap::BoomHashMap;

use super::{AllIndexFiles, IndexEntry, ReadIndex};
use crate::backend::ReadBackend;
use crate::id::Id;

pub struct BoomIndex(BoomHashMap<Id, IndexEntry>);

impl BoomIndex {
    pub fn from_all_indexfiles<BE: ReadBackend>(aif: AllIndexFiles<BE>) -> Self {
        Self::from_iter(aif.into_iter())
    }
}

impl FromIterator<IndexEntry> for BoomIndex {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = IndexEntry>,
    {
        let mut ids = Vec::new();
        let mut ies = Vec::new();

        for ie in iter {
            ids.push(*ie.id());
            ies.push(ie);
        }

        BoomIndex(BoomHashMap::new(ids, ies))
    }
}

impl ReadIndex for BoomIndex {
    fn get_id(&self, id: &Id) -> Option<IndexEntry> {
        self.0.get(id).map(IndexEntry::clone)
    }
}
