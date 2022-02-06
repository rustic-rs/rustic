pub mod indexfiles;

use crate::blob::IndexEntry;
use crate::id::Id;

pub trait ReadIndex {
    fn get_id(&self, id: &Id) -> Option<IndexEntry>;
}
