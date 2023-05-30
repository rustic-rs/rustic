use sha2::{Digest, Sha256};

use crate::id::Id;

#[must_use]
pub fn hash(data: &[u8]) -> Id {
    Id::new(Sha256::digest(data).into())
}
