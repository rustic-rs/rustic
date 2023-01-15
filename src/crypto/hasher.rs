use sha2::{Digest, Sha256};

use crate::id::Id;

pub fn hash(data: &[u8]) -> Id {
    Id::new(Sha256::digest(data).into())
}

pub struct Hasher(Sha256);

impl Hasher {
    pub fn new() -> Self {
        Self(Sha256::new())
    }

    pub fn reset(&mut self) {
        self.0.reset();
    }

    pub fn update(&mut self, data: &[u8]) {
        self.0.update(data);
    }

    pub fn finalize(&mut self) -> Id {
        Id::new(self.0.finalize_reset().into())
    }
}
