use sha2::{Digest, Sha256};

use crate::id::Id;

/// Hashes the given data.
///
/// # Arguments
///
/// * `data` - The data to hash.
///
/// # Returns
///
/// The hash Id of the data.
#[must_use]
pub fn hash(data: &[u8]) -> Id {
    Id::new(Sha256::digest(data).into())
}
