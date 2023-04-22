use std::num::NonZeroU32;

use super::{BlobType, IndexEntry, ReadIndex};
use crate::blob::BlobTypeMap;
use crate::id::Id;
use crate::repofile::{IndexBlob, IndexPack};
use rayon::prelude::*;

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

#[derive(Debug)]
enum EntriesVariants {
    None,
    Ids(Vec<Id>),
    FullEntries(Vec<SortedEntry>),
}

impl Default for EntriesVariants {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Default)]
pub(crate) struct TypeIndexCollector {
    packs: Vec<(Id, u32)>,
    entries: EntriesVariants,
    total_size: u64,
}

#[derive(Default)]
pub(crate) struct IndexCollector(BlobTypeMap<TypeIndexCollector>);

pub struct PackIndexes {
    c: Index,
    tpe: BlobType,
    idx: BlobTypeMap<(usize, usize)>,
}

#[derive(Debug)]
pub(crate) struct TypeIndex {
    packs: Vec<Id>,
    entries: EntriesVariants,
    total_size: u64,
}

#[derive(Debug)]
pub struct Index(BlobTypeMap<TypeIndex>);

impl IndexCollector {
    pub fn new(tpe: IndexType) -> Self {
        let mut collector = Self::default();

        collector.0[BlobType::Tree].entries = EntriesVariants::FullEntries(Vec::new());
        collector.0[BlobType::Data].entries = match tpe {
            IndexType::OnlyTrees => EntriesVariants::None,
            IndexType::FullTrees => EntriesVariants::Ids(Vec::new()),
            IndexType::Full => EntriesVariants::FullEntries(Vec::new()),
        };

        collector
    }

    pub fn tree_packs(&self) -> &Vec<(Id, u32)> {
        &self.0[BlobType::Tree].packs
    }

    pub fn data_packs(&self) -> &Vec<(Id, u32)> {
        &self.0[BlobType::Data].packs
    }

    // Turns Collector into an index by sorting the entries by ID.
    pub fn into_index(self) -> Index {
        Index(self.0.map(|_, mut tc| {
            match &mut tc.entries {
                EntriesVariants::None => {}
                EntriesVariants::Ids(ids) => ids.par_sort_unstable(),
                EntriesVariants::FullEntries(entries) => entries.par_sort_unstable_by_key(|e| e.id),
            };

            let packs = tc.packs.into_iter().map(|(id, _)| id).collect();
            TypeIndex {
                packs,
                entries: tc.entries,
                total_size: tc.total_size,
            }
        }))
    }
}

impl Extend<IndexPack> for IndexCollector {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = IndexPack>,
    {
        for p in iter {
            let len = p.blobs.len();
            let blob_type = p.blob_type();
            let size = p.pack_size();

            let idx = self.0[blob_type].packs.len();
            self.0[blob_type].packs.push((p.id, size));

            self.0[blob_type].total_size += u64::from(size);

            match &mut self.0[blob_type].entries {
                EntriesVariants::None => {}
                EntriesVariants::Ids(ids) => ids.reserve(len),
                EntriesVariants::FullEntries(entries) => entries.reserve(len),
            };

            for blob in &p.blobs {
                let be = SortedEntry {
                    id: blob.id,
                    pack_idx: idx,
                    offset: blob.offset,
                    length: blob.length,
                    uncompressed_length: blob.uncompressed_length,
                };
                match &mut self.0[blob_type].entries {
                    EntriesVariants::None => {}
                    EntriesVariants::Ids(ids) => ids.push(blob.id),
                    EntriesVariants::FullEntries(entries) => entries.push(be),
                };
            }
        }
    }
}

impl Iterator for PackIndexes {
    type Item = IndexPack;

    fn next(&mut self) -> Option<Self::Item> {
        let (pack_idx, idx) = loop {
            let (pack_idx, idx) = &mut self.idx[self.tpe];
            if *pack_idx >= self.c.0[self.tpe].packs.len() {
                if self.tpe == BlobType::Data {
                    return None;
                } else {
                    self.tpe = BlobType::Data;
                }
            } else {
                break (pack_idx, idx);
            }
        };

        let mut pack = IndexPack {
            id: self.c.0[self.tpe].packs[*pack_idx],
            ..Default::default()
        };

        if let EntriesVariants::FullEntries(entries) = &self.c.0[self.tpe].entries {
            while *idx < entries.len() && entries[*idx].pack_idx == *pack_idx {
                let entry = &entries[*idx];
                pack.blobs.push(IndexBlob {
                    id: entry.id,
                    tpe: self.tpe,
                    offset: entry.offset,
                    length: entry.length,
                    uncompressed_length: entry.uncompressed_length,
                });
                *idx += 1;
            }
        }
        *pack_idx += 1;

        Some(pack)
    }
}

impl IntoIterator for Index {
    type Item = IndexPack;
    type IntoIter = PackIndexes;

    // Turns Collector into an iterator yielding PackIndex by sorting the entries by pack.
    fn into_iter(mut self) -> Self::IntoIter {
        for (_, tc) in self.0.iter_mut() {
            if let EntriesVariants::FullEntries(entries) = &mut tc.entries {
                entries.par_sort_unstable_by(|e1, e2| e1.pack_idx.cmp(&e2.pack_idx));
            }
        }
        PackIndexes {
            c: Index(self.0.map(|_, mut tc| {
                if let EntriesVariants::FullEntries(entries) = &mut tc.entries {
                    entries.par_sort_unstable_by(|e1, e2| e1.pack_idx.cmp(&e2.pack_idx));
                }

                TypeIndex {
                    packs: tc.packs,
                    entries: tc.entries,
                    total_size: tc.total_size,
                }
            })),
            tpe: BlobType::Tree,
            idx: BlobTypeMap::default(),
        }
    }
}

impl ReadIndex for Index {
    fn get_id(&self, blob_type: BlobType, id: &Id) -> Option<IndexEntry> {
        let vec = match &self.0[blob_type].entries {
            EntriesVariants::FullEntries(entries) => entries,
            _ => {
                // get_id() only gives results if index contains full entries
                return None;
            }
        };

        vec.binary_search_by_key(id, |e| e.id).ok().map(|index| {
            let be = &vec[index];
            IndexEntry::new(
                blob_type,
                self.0[blob_type].packs[be.pack_idx],
                be.offset,
                be.length,
                be.uncompressed_length,
            )
        })
    }

    fn total_size(&self, blob_type: BlobType) -> u64 {
        self.0[blob_type].total_size
    }

    fn has(&self, blob_type: BlobType, id: &Id) -> bool {
        match &self.0[blob_type].entries {
            EntriesVariants::FullEntries(entries) => {
                entries.binary_search_by_key(id, |e| e.id).is_ok()
            }
            EntriesVariants::Ids(ids) => ids.binary_search(id).is_ok(),
            // has() only gives results if index contains full entries or ids
            EntriesVariants::None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repofile::IndexFile;

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
            assert!(!index.has(BlobType::Data, &id));
            assert!(index.get_id(BlobType::Data, &id).is_none());
            assert!(!index.has(BlobType::Tree, &id));
            assert!(index.get_id(BlobType::Tree, &id).is_none());

            let id = parse("aac5e908151e5652b7570108127b96e6bae22bcdda1d3d867f63ed1555fc8aef");
            assert!(!index.has(BlobType::Data, &id,));
            assert!(index.get_id(BlobType::Data, &id).is_none());
            assert!(!index.has(BlobType::Tree, &id));
            assert!(index.get_id(BlobType::Tree, &id).is_none());

            let id = parse("2ef8decbd2a17d9bfb1b35cfbdcd368175ea86d05dd93a4751fdacbe5213e611");
            assert!(!index.has(BlobType::Data, &id));
            assert!(index.get_id(BlobType::Data, &id).is_none());
            assert!(index.has(BlobType::Tree, &id));
            assert_eq!(
                index.get_id(BlobType::Tree, &id),
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
        assert!(!index.has(BlobType::Data, &id));
        assert!(index.get_id(BlobType::Data, &id).is_none());
        assert!(!index.has(BlobType::Tree, &id));
        assert!(index.get_id(BlobType::Tree, &id).is_none());

        let id = parse("620b2cef43d4c7aab3d7c911a3c0e872d2e0e70f170201002b8af8fb98c59da5");
        assert!(!index.has(BlobType::Data, &id));
        assert!(index.get_id(BlobType::Data, &id).is_none());
        assert!(!index.has(BlobType::Tree, &id));
        assert!(index.get_id(BlobType::Tree, &id).is_none());
    }

    #[test]
    fn full_trees() {
        let index = index(IndexType::FullTrees);

        let id = parse("fac5e908151e565267570108127b96e6bae22bcdda1d3d867f63ed1555fc8aef");
        assert!(index.has(BlobType::Data, &id));
        assert!(index.get_id(BlobType::Data, &id).is_none());
        assert!(!index.has(BlobType::Tree, &id));
        assert!(index.get_id(BlobType::Tree, &id).is_none());

        let id = parse("620b2cef43d4c7aab3d7c911a3c0e872d2e0e70f170201002b8af8fb98c59da5");
        assert!(index.has(BlobType::Data, &id));
        assert!(index.get_id(BlobType::Data, &id).is_none());
        assert!(!index.has(BlobType::Tree, &id));
        assert!(index.get_id(BlobType::Tree, &id).is_none());
    }

    #[test]
    fn full() {
        let index = index(IndexType::Full);

        let id = parse("fac5e908151e565267570108127b96e6bae22bcdda1d3d867f63ed1555fc8aef");
        assert!(index.has(BlobType::Data, &id));
        assert_eq!(
            index.get_id(BlobType::Data, &id),
            Some(IndexEntry {
                blob_type: BlobType::Data,
                pack: parse("217f145b63fbc10267f5a686186689ea3389bed0d6a54b50ffc84d71f99eb7fa"),
                offset: 5185,
                length: 2095,
                uncompressed_length: Some(NonZeroU32::new(6411).unwrap()),
            }),
        );
        assert!(!index.has(BlobType::Tree, &id));
        assert!(index.get_id(BlobType::Tree, &id).is_none());

        let id = parse("620b2cef43d4c7aab3d7c911a3c0e872d2e0e70f170201002b8af8fb98c59da5");
        assert!(index.has(BlobType::Data, &id));
        assert_eq!(
            index.get_id(BlobType::Data, &id),
            Some(IndexEntry {
                blob_type: BlobType::Data,
                pack: parse("3b25ec6d16401c31099c259311562160b1b5efbcf70bd69d0463104d3b8148fc"),
                offset: 6324,
                length: 1413,
                uncompressed_length: Some(NonZeroU32::new(3752).unwrap()),
            }),
        );
        assert!(!index.has(BlobType::Tree, &id));
        assert!(index.get_id(BlobType::Tree, &id).is_none());
    }
}
