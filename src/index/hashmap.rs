use std::collections::HashMap;

use super::{BlobType, IndexEntry, ReadIndex};
use crate::id::Id;
use crate::repo::IndexPack;

#[derive(Debug)]
struct HashMapEntry {
    pack_idx: usize,
    offset: u32,
    length: u32,
}

pub(super) struct HashMapIndex {
    packs: Vec<Id>,
    hash: HashMap<BlobType, HashMap<Id, HashMapEntry>>,
}

impl FromIterator<IndexPack> for HashMapIndex {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = IndexPack>,
    {
        let mut packs = Vec::new();
        let mut map: HashMap<BlobType, HashMap<_, _>> = HashMap::new();

        for i in iter {
            let idx = packs.len();
            packs.push(*i.id());

            for blob in i.blobs() {
                let be = HashMapEntry {
                    pack_idx: idx,
                    offset: *blob.offset(),
                    length: *blob.length(),
                };
                let entry = map.entry(*blob.tpe()).or_default();
                entry.insert(*blob.id(), be);
            }
        }

        Self { packs, hash: map }
    }
}

impl ReadIndex for HashMapIndex {
    fn get_id(&self, tpe: &BlobType, id: &Id) -> Option<IndexEntry> {
        self.hash
            .get(tpe)
            .map(|entry| {
                entry
                    .get(id)
                    .map(|be| IndexEntry::new(self.packs[be.pack_idx], be.offset, be.length))
            })
            .flatten()
    }
}
