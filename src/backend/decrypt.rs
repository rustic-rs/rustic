use std::io::{Cursor, Read};

use thiserror::Error;

use super::{FileType, Id, ReadBackend, WriteBackend};
use crate::crypto::{hash, CryptoKey};

pub trait DecryptReadBackend: ReadBackend {}

pub trait DecryptWriteBackend: WriteBackend {
    type Key: CryptoKey;
    fn key(&self) -> &Self::Key;
}

/// RepoError describes the errors that can be returned by accessing this repository
#[derive(Error, Debug)]
pub enum RepoError<R, C> {
    /// Represents an error while encrypting/decrypting.
    #[error("Crypto error")]
    CryptoError(C),

    /// Represents another error from the embedded repository.
    #[error("Repo error")]
    RepoError(#[from] R),
}

#[derive(Clone)]
pub struct DecryptBackend<R, C> {
    backend: R,
    key: C,
}

impl<R: ReadBackend, C> DecryptBackend<R, C> {
    pub fn new(be: &R, key: C) -> Self {
        Self {
            backend: be.clone(),
            key,
        }
    }
}

impl<R: WriteBackend, C: CryptoKey> DecryptWriteBackend for DecryptBackend<R, C> {
    type Key = C;
    fn key(&self) -> &Self::Key {
        &self.key
    }
}

impl<R: ReadBackend, C: CryptoKey> DecryptReadBackend for DecryptBackend<R, C> {}

impl<R: ReadBackend, C: CryptoKey> ReadBackend for DecryptBackend<R, C> {
    type Error = RepoError<R::Error, C::CryptoError>;

    fn location(&self) -> &str {
        self.backend.location()
    }

    fn list(&self, tpe: FileType) -> Result<Vec<Id>, Self::Error> {
        self.backend.list(tpe).map_err(RepoError::RepoError)
    }

    fn list_with_size(&self, tpe: FileType) -> Result<Vec<(Id, u32)>, Self::Error> {
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

impl<R: WriteBackend, C: CryptoKey> WriteBackend for DecryptBackend<R, C> {
    type Error = RepoError<R::Error, C::CryptoError>;

    fn write_full(&self, tpe: FileType, id: &Id, r: &mut impl Read) -> Result<(), Self::Error> {
        self.backend.write_full(tpe, id, r)?;
        Ok(())
    }

    fn hash_write_full(&self, tpe: FileType, data: &[u8]) -> Result<Id, Self::Error> {
        let data = self
            .key
            .encrypt_data(data)
            .map_err(RepoError::CryptoError)?;
        let id = hash(&data);
        self.write_full(tpe, &id, &mut Cursor::new(data))?;
        Ok(id)
    }
}
