use thiserror::Error;

use super::{FileType, Id, IdWithSize, ReadBackend};
use crate::crypto::{CryptoError, Key};

/// RepoError describes the errors that can be returned by accessing this repository
#[derive(Error, Debug)]
pub enum RepoError<E> {
    /// Represents an error while decrypting.
    #[error("Decryption error")]
    CryptoError(CryptoError),

    /// Represents another error from the embedded repository.
    #[error("Repo error")]
    RepoError(#[from] E),
}

#[derive(Clone)]
pub struct DecryptBackend<R> {
    backend: R,
    key: Key,
}

impl<R: ReadBackend> DecryptBackend<R> {
    pub fn new(be: &R, key: Key) -> Self {
        Self {
            backend: be.clone(),
            key,
        }
    }
}

impl<R: ReadBackend> ReadBackend for DecryptBackend<R> {
    type Error = RepoError<R::Error>;

    fn location(&self) -> &str {
        self.backend.location()
    }

    fn list(&self, tpe: FileType) -> Result<Vec<Id>, Self::Error> {
        self.backend.list(tpe).map_err(RepoError::RepoError)
    }

    fn list_with_size(&self, tpe: FileType) -> Result<Vec<IdWithSize>, Self::Error> {
        self.backend
            .list_with_size(tpe)
            .map_err(RepoError::RepoError)
    }

    fn read_full(&self, tpe: FileType, id: Id) -> Result<Vec<u8>, Self::Error> {
        self.key
            .decrypt_data(&self.backend.read_full(tpe, id)?)
            .map_err(RepoError::CryptoError)
    }

    fn read_partial(
        &self,
        tpe: FileType,
        id: Id,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>, Self::Error> {
        self.key
            .decrypt_data(&self.backend.read_partial(tpe, id, offset, length)?)
            .map_err(RepoError::CryptoError)
    }
}
