use thiserror::Error;

use super::{FileType, Id, ReadBackend};
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

pub struct DecryptBackend<'a, R> {
    backend: &'a R,
    key: Key,
}

impl<'a, R: ReadBackend> DecryptBackend<'a, R> {
    pub fn new(be: &'a R, key: Key) -> Self {
        Self {
            backend: be,
            key: key,
        }
    }
}

impl<'a, R: ReadBackend> ReadBackend for DecryptBackend<'a, R> {
    type Error = RepoError<R::Error>;

    fn location(&self) -> &str {
        self.backend.location()
    }

    fn list(&self, tpe: FileType) -> Result<Vec<Id>, Self::Error> {
        self.backend
            .list(tpe)
            .map_err(|err| RepoError::RepoError(err))
    }

    fn read_full(&self, tpe: FileType, id: Id) -> Result<Vec<u8>, Self::Error> {
        self.key
            .decrypt_data(&self.backend.read_full(tpe, id)?)
            .map_err(|err| RepoError::CryptoError(err))
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
            .map_err(|err| RepoError::CryptoError(err))
    }
}
