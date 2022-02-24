use std::collections::HashMap;

use super::{BlobType, IndexEntry, ReadIndex};
use crate::id::Id;
use crate::repo::IndexPack;

#[derive(Debug)]
struct HashMapEntry {
    pack_idx: usize,
    tpe: BlobType,
    offset: u32,
    length: u32,
}

pub(super) struct HashMapIndex {
    packs: Vec<Id>,
    hash: HashMap<Id, HashMapEntry>,
}

impl FromIterator<IndexPack> for HashMapIndex {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = IndexPack>,
    {
        let mut packs = Vec::new();
        let mut map = HashMap::new();

        for i in iter {
            let idx = packs.len();
            packs.push(*i.id());

            let len = i.blobs().len();
            map.reserve(len);
            for blob in i.blobs() {
                let be = HashMapEntry {
                    pack_idx: idx,
                    tpe: *blob.tpe(),
                    offset: *blob.offset(),
                    length: *blob.length(),
                };
                map.insert(*blob.id(), be);
            }
        }

        Self { packs, hash: map }
    }
}

impl ReadIndex for HashMapIndex {
    fn get_id(&self, id: &Id) -> Option<IndexEntry> {
        self.hash
            .get(id)
            .map(|be| IndexEntry::new(self.packs[be.pack_idx], be.tpe, be.offset, be.length))
    }
}
