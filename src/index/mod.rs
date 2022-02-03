use crate::blob::{Blob, IndexEntry};
use crate::Id;

pub trait ReadIndex {
    fn iter(&self) -> Box<dyn Iterator<Item = IndexEntry> + '_>;

    fn get_id(&self, id: Id) -> Option<IndexEntry> {
        self.iter().find(|e| e.bi.blob.id == id)
    }

    fn get_blob(&self, blob: Blob) -> Option<IndexEntry> {
        self.iter().find(|e| e.bi.blob == blob)
    }
}
