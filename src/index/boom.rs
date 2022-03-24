use boomphf::hashmap::BoomHashMap;
use futures::{Stream, StreamExt};

use super::{BlobType, IndexEntry, ReadIndex};
use crate::id::Id;
use crate::repo::IndexFile;

#[derive(Debug)]
struct BoomEntry {
    pack_idx: usize,
    offset: u32,
    length: u32,
}

pub(super) struct BoomIndex {
    packs: Vec<Id>,
    tree: BoomHashMap<Id, BoomEntry>,
    data: BoomHashSetMap,
}

enum BoomHashSetMap {
    Set(BoomHashMap<Id, ()>),
    Map(BoomHashMap<Id, BoomEntry>),
}

async fn from_stream<T>(
    mut stream: T,
    full_data: bool,
) -> (Vec<Id>, Vec<Id>, Vec<BoomEntry>, Vec<Id>, Vec<BoomEntry>)
where
    T: Stream<Item = IndexFile> + Unpin,
{
    let mut packs = Vec::new();
    let mut tree_ids = Vec::new();
    let mut tree_bes = Vec::new();
    let mut data_ids = Vec::new();
    let mut data_bes = Vec::new();

    while let Some(index) = stream.next().await {
        for i in index.dissolve().1 {
            let idx = packs.len();
            packs.push(*i.id());
            let len = i.blobs().len();
            if i.blobs()[0].tpe() == &BlobType::Data {
                data_ids.reserve(len);
                if full_data {
                    data_bes.reserve(len);
                }
            } else {
                tree_ids.reserve(len);
                tree_bes.reserve(len);
            }

            for blob in i.blobs() {
                let be = BoomEntry {
                    pack_idx: idx,
                    offset: *blob.offset(),
                    length: *blob.length(),
                };
                match blob.tpe() {
                    BlobType::Tree => {
                        tree_ids.push(*blob.id());
                        tree_bes.push(be);
                    }
                    BlobType::Data => {
                        data_ids.push(*blob.id());
                        if full_data {
                            data_bes.push(be);
                        }
                    }
                }
            }
        }
    }
    (packs, tree_ids, tree_bes, data_ids, data_bes)
}

impl BoomIndex {
    pub async fn only_full_trees<T>(stream: T) -> Self
    where
        T: Stream<Item = IndexFile> + Unpin,
    {
        let (packs, tree_ids, tree_bes, data_ids, _data_bes) = from_stream(stream, false).await;
        let len = data_ids.len();

        Self {
            packs,
            tree: BoomHashMap::new(tree_ids, tree_bes),
            data: BoomHashSetMap::Set(BoomHashMap::new(data_ids, vec![(); len])),
        }
    }

    pub async fn full<T>(stream: T) -> Self
    where
        T: Stream<Item = IndexFile> + Unpin,
    {
        let (packs, tree_ids, tree_bes, data_ids, data_bes) = from_stream(stream, true).await;

        Self {
            packs,
            tree: BoomHashMap::new(tree_ids, tree_bes),
            data: BoomHashSetMap::Map(BoomHashMap::new(data_ids, data_bes)),
        }
    }
}

impl ReadIndex for BoomIndex {
    fn get_id(&self, tpe: &BlobType, id: &Id) -> Option<IndexEntry> {
        let boom = match (tpe, &self.data) {
            (BlobType::Tree, _) => &self.tree,
            (BlobType::Data, BoomHashSetMap::Map(data)) => data,
            (BlobType::Data, BoomHashSetMap::Set(_)) => {
                return None;
            }
        };
        boom.get(id)
            .map(|be| IndexEntry::new(self.packs[be.pack_idx], be.offset, be.length))
    }

    fn has(&self, tpe: &BlobType, id: &Id) -> bool {
        match (tpe, &self.data) {
            (BlobType::Tree, _) => self.tree.get(id).is_some(),
            (BlobType::Data, BoomHashSetMap::Map(data)) => data.get(id).is_some(),
            (BlobType::Data, BoomHashSetMap::Set(data)) => data.get(id).is_some(),
        }
    }
}
