use futures::{Stream, StreamExt};

use super::{BlobType, IndexEntry, ReadIndex};
use crate::id::Id;
use crate::repo::IndexFile;

#[derive(Debug, PartialEq, Eq)]
struct BinarySortedEntry {
    id: Id,
    pack_idx: usize,
    offset: u32,
    length: u32,
}

pub(super) struct BinarySortedIndex {
    packs: Vec<Id>,
    tree: Vec<BinarySortedEntry>,
    data: BinarySortedHashSetMap,
}

enum BinarySortedHashSetMap {
    Set(Vec<Id>),
    Map(Vec<BinarySortedEntry>),
}

async fn from_stream<T>(
    mut stream: T,
    full_data: bool,
) -> (
    Vec<Id>,
    Vec<BinarySortedEntry>,
    Vec<BinarySortedEntry>,
    Vec<Id>,
)
where
    T: Stream<Item = IndexFile> + Unpin,
{
    let mut packs = Vec::new();
    let mut tree = Vec::new();
    let mut data = Vec::new();
    let mut data_id = Vec::new();

    while let Some(index) = stream.next().await {
        for p in index.packs {
            let idx = packs.len();
            packs.push(p.id);
            let len = p.blobs.len();
            if p.blob_type() == BlobType::Data {
                if full_data {
                    data.reserve(len);
                } else {
                    data_id.reserve(len);
                }
            } else {
                tree.reserve(len);
            }

            for blob in p.blobs {
                let be = BinarySortedEntry {
                    id: blob.id,
                    pack_idx: idx,
                    offset: blob.offset,
                    length: blob.length,
                };
                match blob.tpe {
                    BlobType::Tree => {
                        tree.push(be);
                    }
                    BlobType::Data => {
                        if full_data {
                            data.push(be);
                        } else {
                            data_id.push(blob.id);
                        }
                    }
                }
            }
        }
    }
    (packs, tree, data, data_id)
}

impl BinarySortedIndex {
    pub async fn only_full_trees<T>(stream: T) -> Self
    where
        T: Stream<Item = IndexFile> + Unpin,
    {
        let (packs, mut tree, _, mut data) = from_stream(stream, false).await;

        tree.sort_unstable_by_key(|e| e.id);
        data.sort_unstable();

        Self {
            packs,
            tree,
            data: BinarySortedHashSetMap::Set(data),
        }
    }

    pub async fn full<T>(stream: T) -> Self
    where
        T: Stream<Item = IndexFile> + Unpin,
    {
        let (packs, mut tree, mut data, _) = from_stream(stream, true).await;

        tree.sort_unstable_by_key(|e| e.id);
        data.sort_unstable_by_key(|e| e.id);

        Self {
            packs,
            tree,
            data: BinarySortedHashSetMap::Map(data),
        }
    }
}

impl ReadIndex for BinarySortedIndex {
    fn get_id(&self, tpe: &BlobType, id: &Id) -> Option<IndexEntry> {
        let vec = match (tpe, &self.data) {
            (BlobType::Tree, _) => &self.tree,
            (BlobType::Data, BinarySortedHashSetMap::Map(data)) => data,
            (BlobType::Data, BinarySortedHashSetMap::Set(_)) => {
                return None;
            }
        };
        vec.binary_search_by_key(id, |e| e.id).ok().map(|index| {
            let be = &vec[index];
            IndexEntry::new(self.packs[be.pack_idx], be.offset, be.length)
        })
    }

    fn has(&self, tpe: &BlobType, id: &Id) -> bool {
        match (tpe, &self.data) {
            (BlobType::Tree, _) => self.tree.binary_search_by_key(id, |e| e.id).is_ok(),
            (BlobType::Data, BinarySortedHashSetMap::Map(data)) => {
                data.binary_search_by_key(id, |e| e.id).is_ok()
            }
            (BlobType::Data, BinarySortedHashSetMap::Set(data)) => data.binary_search(id).is_ok(),
        }
    }
}
