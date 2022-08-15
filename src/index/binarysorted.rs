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
    total_tree_size: u64,
    total_data_size: u64,
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
            total_tree_size: 0,
            total_data_size: 0,
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
            total_tree_size: self.total_tree_size,
            total_data_size: self.total_data_size,
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
            let blob_type = p.blob_type();

            match blob_type {
                BlobType::Tree => self.total_tree_size += p.pack_size() as u64,
                BlobType::Data => self.total_data_size += p.pack_size() as u64,
            }

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
                match (blob.tpe, &mut self.data) {
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
    total_tree_size: u64,
    total_data_size: u64,
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
                *tpe,
                self.packs[be.pack_idx],
                be.offset,
                be.length,
                be.uncompressed_length,
            )
        })
    }

    fn total_size(&self, tpe: &BlobType) -> u64 {
        match tpe {
            BlobType::Tree => self.total_tree_size,
            BlobType::Data => self.total_data_size,
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::IndexFile;

    const JSON_INDEX: &str = r#"
{"packs":[{"id":"217f145b63fbc10267f5a686186689ea3389bed0d6a54b50ffc84d71f99eb7fa",
           "blobs":[{"id":"a3e048f1073299310981d8f5447861df0eca26a706645b5e2fa355c31c2205ed",
                     "type":"data",
                     "offset":0,
                     "length":2869,
                     "uncompressed_length":9987},
                    {"id":"458c0b9b656a6593b7ba85ecdbfe85d6cb32af70c2e9c5fd1871cf3dccc39044",
                     "type":"data",
                     "offset":2869,
                     "length":2316,
                     "uncompressed_length":7370},
                    {"id":"fac5e908151e565267570108127b96e6bae22bcdda1d3d867f63ed1555fc8aef",
                     "type":"data",
                     "offset":5185,
                     "length":2095,
                     "uncompressed_length":6411}
                   ]},
          {"id":"3b25ec6d16401c31099c259311562160b1b5efbcf70bd69d0463104d3b8148fc",
           "blobs":[{"id":"620b2cef43d4c7aab3d7c911a3c0e872d2e0e70f170201002b8af8fb98c59da5",
                     "type":"data",
                     "offset":6324,
                     "length":1413,
                     "uncompressed_length":3752},
                    {"id":"ee67585c7c53324e74537ab7aa44f889c0767c1b67e7e336fae6204aef2d4c73",
                     "type":"data",
                     "offset":7737,
                     "length":7686,
                     "uncompressed_length":29928},
                    {"id":"8aaa5f7f6c7b4a5ea5c70a744bf40002c54542e5a573c13d41ac9c8b17f426c1",
                     "type":"data",
                     "offset":15423,
                     "length":1419,
                     "uncompressed_length":3905},
                   {"id":"f2ca1bb6c7e907d06dafe4687e579fce76b37e4e93b7605022da52e6ccc26fd2",
                    "type":"data",
                    "offset":16842,
                    "length":46,
                    "uncompressed_length":5}
                  ]},
         {"id":"8431a27d38dd7d192dc37abd43a85d6dc4298de72fc8f583c5d7cdd09fa47274",
          "blobs":[{"id":"3b25ec6d16401c31099c259311562160b1b5efbcf70bd69d0463104d3b8148fc",
                    "type":"tree",
                    "offset":0,
                    "length":794,
                    "uncompressed_length":3030},
                   {"id":"2ef8decbd2a17d9bfb1b35cfbdcd368175ea86d05dd93a4751fdacbe5213e611",
                    "type":"tree",
                    "offset":794,
                    "length":592,
                    "uncompressed_length":1912}
                  ]}
        ]}"#;

    fn index(it: IndexType) -> Index {
        let index: IndexFile = serde_json::from_str(JSON_INDEX).unwrap();
        let mut collector = IndexCollector::new(it);
        collector.extend(index.packs);
        collector.into_index()
    }

    fn parse(s: &str) -> Id {
        Id::from_hex(s).unwrap()
    }

    #[test]
    fn all_index_types() {
        for it in [IndexType::OnlyTrees, IndexType::FullTrees, IndexType::Full] {
            let index = index(it);

            let id = parse("0000000000000000000000000000000000000000000000000000000000000000");
            assert!(!index.has(&BlobType::Data, &id));
            assert!(index.get_id(&BlobType::Data, &id).is_none());
            assert!(!index.has(&BlobType::Tree, &id));
            assert!(index.get_id(&BlobType::Tree, &id).is_none());

            let id = parse("aac5e908151e5652b7570108127b96e6bae22bcdda1d3d867f63ed1555fc8aef");
            assert!(!index.has(&BlobType::Data, &id,));
            assert!(index.get_id(&BlobType::Data, &id).is_none());
            assert!(!index.has(&BlobType::Tree, &id));
            assert!(index.get_id(&BlobType::Tree, &id).is_none());

            let id = parse("2ef8decbd2a17d9bfb1b35cfbdcd368175ea86d05dd93a4751fdacbe5213e611");
            assert!(!index.has(&BlobType::Data, &id));
            assert!(index.get_id(&BlobType::Data, &id).is_none());
            assert!(index.has(&BlobType::Tree, &id));
            assert_eq!(
                index.get_id(&BlobType::Tree, &id),
                Some(IndexEntry {
                    blob_type: BlobType::Tree,
                    pack: parse("8431a27d38dd7d192dc37abd43a85d6dc4298de72fc8f583c5d7cdd09fa47274"),
                    offset: 794,
                    length: 592,
                    uncompressed_length: Some(NonZeroU32::new(1912).unwrap()),
                }),
            );
        }
    }

    #[test]
    fn only_trees() {
        let index = index(IndexType::OnlyTrees);

        let id = parse("fac5e908151e565267570108127b96e6bae22bcdda1d3d867f63ed1555fc8aef");
        assert!(!index.has(&BlobType::Data, &id));
        assert!(index.get_id(&BlobType::Data, &id).is_none());
        assert!(!index.has(&BlobType::Tree, &id));
        assert!(index.get_id(&BlobType::Tree, &id).is_none());

        let id = parse("620b2cef43d4c7aab3d7c911a3c0e872d2e0e70f170201002b8af8fb98c59da5");
        assert!(!index.has(&BlobType::Data, &id));
        assert!(index.get_id(&BlobType::Data, &id).is_none());
        assert!(!index.has(&BlobType::Tree, &id));
        assert!(index.get_id(&BlobType::Tree, &id).is_none());
    }

    #[test]
    fn full_trees() {
        let index = index(IndexType::FullTrees);

        let id = parse("fac5e908151e565267570108127b96e6bae22bcdda1d3d867f63ed1555fc8aef");
        assert!(index.has(&BlobType::Data, &id));
        assert!(index.get_id(&BlobType::Data, &id).is_none());
        assert!(!index.has(&BlobType::Tree, &id));
        assert!(index.get_id(&BlobType::Tree, &id).is_none());

        let id = parse("620b2cef43d4c7aab3d7c911a3c0e872d2e0e70f170201002b8af8fb98c59da5");
        assert!(index.has(&BlobType::Data, &id));
        assert!(index.get_id(&BlobType::Data, &id).is_none());
        assert!(!index.has(&BlobType::Tree, &id));
        assert!(index.get_id(&BlobType::Tree, &id).is_none());
    }

    #[test]
    fn full() {
        let index = index(IndexType::Full);

        let id = parse("fac5e908151e565267570108127b96e6bae22bcdda1d3d867f63ed1555fc8aef");
        assert!(index.has(&BlobType::Data, &id));
        assert_eq!(
            index.get_id(&BlobType::Data, &id),
            Some(IndexEntry {
                blob_type: BlobType::Data,
                pack: parse("217f145b63fbc10267f5a686186689ea3389bed0d6a54b50ffc84d71f99eb7fa"),
                offset: 5185,
                length: 2095,
                uncompressed_length: Some(NonZeroU32::new(6411).unwrap()),
            }),
        );
        assert!(!index.has(&BlobType::Tree, &id));
        assert!(index.get_id(&BlobType::Tree, &id).is_none());

        let id = parse("620b2cef43d4c7aab3d7c911a3c0e872d2e0e70f170201002b8af8fb98c59da5");
        assert!(index.has(&BlobType::Data, &id));
        assert_eq!(
            index.get_id(&BlobType::Data, &id),
            Some(IndexEntry {
                blob_type: BlobType::Data,
                pack: parse("3b25ec6d16401c31099c259311562160b1b5efbcf70bd69d0463104d3b8148fc"),
                offset: 6324,
                length: 1413,
                uncompressed_length: Some(NonZeroU32::new(3752).unwrap()),
            }),
        );
        assert!(!index.has(&BlobType::Tree, &id));
        assert!(index.get_id(&BlobType::Tree, &id).is_none());
    }
}
