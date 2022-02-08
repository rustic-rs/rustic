use boomphf::hashmap::BoomHashMap;

use super::{BlobType, IndexEntry, ReadIndex};
use crate::id::Id;
use crate::repo::IndexPack;

#[derive(Debug)]
struct BoomEntry {
    pack_idx: usize,
    tpe: BlobType,
    offset: u32,
    length: u32,
}

pub struct BoomIndex {
    packs: Vec<Id>,
    boom: BoomHashMap<Id, BoomEntry>,
}

impl FromIterator<IndexPack> for BoomIndex {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = IndexPack>,
    {
        let mut packs = Vec::new();
        let mut ids = Vec::new();
        let mut bes = Vec::new();

        for i in iter {
            let idx = packs.len();
            packs.push(*i.id());

            let len = i.blobs().len();
            ids.reserve(len);
            bes.reserve(len);
            for blob in i.blobs() {
                let be = BoomEntry {
                    pack_idx: idx,
                    tpe: *blob.tpe(),
                    offset: *blob.offset(),
                    length: *blob.length(),
                };
                ids.push(*blob.id());
                bes.push(be);
            }
        }

        Self {
            packs,
            boom: BoomHashMap::new(ids, bes),
        }
    }
}

impl ReadIndex for BoomIndex {
    fn get_id(&self, id: &Id) -> Option<IndexEntry> {
        self.boom
            .get(id)
            .map(|be| IndexEntry::new(self.packs[be.pack_idx], be.tpe, be.offset, be.length))
    }
}
