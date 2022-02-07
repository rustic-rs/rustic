pub mod boom;
pub mod indexfiles;

pub use boom::*;
pub use indexfiles::*;

use crate::blob::IndexEntry;
use crate::id::Id;

pub trait ReadIndex {
    fn get_id(&self, id: &Id) -> Option<IndexEntry>;
}
