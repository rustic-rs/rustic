use std::num::NonZeroU32;

use super::{BlobType, IndexEntry, ReadIndex};
use crate::id::Id;
use crate::repo::IndexPack;

#[derive(Debug, PartialEq, Eq)]
struct SortedEntry {
    id: Id,
    pack_idx: usize,
    offset: u32,
    length: u32,
    uncompressed_length: Option<NonZeroU32>,
}

pub(crate) enum IndexType {
    Full,
    FullTrees,
    OnlyTrees,
}

enum SortedHashSetMap {
    None,
    Set(Vec<Id>),
    Map(Vec<SortedEntry>),
}

pub(crate) struct IndexCollector {
    packs: Vec<Id>,
    tree: Vec<SortedEntry>,
    data: SortedHashSetMap,
}

impl IndexCollector {
    pub fn new(tpe: IndexType) -> Self {
        let data = match tpe {
            IndexType::OnlyTrees => SortedHashSetMap::None,
            IndexType::FullTrees => SortedHashSetMap::Set(Vec::new()),
            IndexType::Full => SortedHashSetMap::Map(Vec::new()),
        };
        Self {
            packs: Vec::new(),
            tree: Vec::new(),
            data,
        }
    }

    pub fn into_index(mut self) -> Index {
        self.tree.sort_unstable_by_key(|e| e.id);
        match &mut self.data {
            SortedHashSetMap::None => {}
            SortedHashSetMap::Set(ids) => ids.sort_unstable(),
            SortedHashSetMap::Map(data) => data.sort_unstable_by_key(|e| e.id),
        };
        Index {
            packs: self.packs,
            tree: self.tree,
            data: self.data,
        }
    }
}

impl Extend<IndexPack> for IndexCollector {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = IndexPack>,
    {
        for p in iter {
            let idx = self.packs.len();
            self.packs.push(p.id);
            let len = p.blobs.len();

            match (p.blob_type(), &mut self.data) {
                (BlobType::Tree, _) => self.tree.reserve(len),
                (BlobType::Data, SortedHashSetMap::None) => {}
                (BlobType::Data, SortedHashSetMap::Set(ids)) => ids.reserve(len),
                (BlobType::Data, SortedHashSetMap::Map(data)) => data.reserve(len),
            };

            for blob in &p.blobs {
                let be = SortedEntry {
                    id: blob.id,
                    pack_idx: idx,
                    offset: blob.offset,
                    length: blob.length,
                    uncompressed_length: blob.uncompressed_length,
                };
                match (p.blob_type(), &mut self.data) {
                    (BlobType::Tree, _) => self.tree.push(be),
                    (BlobType::Data, SortedHashSetMap::None) => {}
                    (BlobType::Data, SortedHashSetMap::Set(ids)) => ids.push(blob.id),
                    (BlobType::Data, SortedHashSetMap::Map(data)) => data.push(be),
                };
            }
        }
    }
}

pub struct Index {
    packs: Vec<Id>,
    tree: Vec<SortedEntry>,
    data: SortedHashSetMap,
}

impl ReadIndex for Index {
    fn get_id(&self, tpe: &BlobType, id: &Id) -> Option<IndexEntry> {
        let vec = match (tpe, &self.data) {
            (BlobType::Tree, _) => &self.tree,
            (BlobType::Data, SortedHashSetMap::Map(data)) => data,
            (BlobType::Data, _) => {
                return None;
            }
        };
        vec.binary_search_by_key(id, |e| e.id).ok().map(|index| {
            let be = &vec[index];
            IndexEntry::new(
                self.packs[be.pack_idx],
                be.offset,
                be.length,
                be.uncompressed_length,
            )
        })
    }

    fn has(&self, tpe: &BlobType, id: &Id) -> bool {
        match (tpe, &self.data) {
            (BlobType::Tree, _) => self.tree.binary_search_by_key(id, |e| e.id).is_ok(),
            (BlobType::Data, SortedHashSetMap::Map(data)) => {
                data.binary_search_by_key(id, |e| e.id).is_ok()
            }
            (BlobType::Data, SortedHashSetMap::Set(data)) => data.binary_search(id).is_ok(),
            (BlobType::Data, SortedHashSetMap::None) => false,
        }
    }
}
