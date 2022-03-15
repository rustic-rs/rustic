use std::io::{Cursor, Read};

use anyhow::Result;

use super::{FileType, Id, ReadBackend, RepoFile, WriteBackend};
use crate::crypto::{hash, CryptoKey};

pub trait DecryptFullBackend: DecryptWriteBackend + DecryptReadBackend {}
impl<T: DecryptWriteBackend + DecryptReadBackend> DecryptFullBackend for T {}

pub trait DecryptReadBackend: ReadBackend {
    fn read_encrypted_full(&self, tpe: FileType, id: &Id) -> Result<Vec<u8>>;
    fn read_encrypted_partial(
        &self,
        tpe: FileType,
        id: &Id,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>>;

    fn get_file<F: RepoFile>(&self, id: &Id) -> Result<F> {
        let data = self.read_encrypted_full(F::TYPE, id)?;
        Ok(serde_json::from_slice(&data)?)
    }
}

pub trait DecryptWriteBackend: WriteBackend {
    type Key: CryptoKey;
    fn key(&self) -> &Self::Key;
    fn hash_write_full(&self, tpe: FileType, data: &[u8]) -> Result<Id>;

    fn save_file<F: RepoFile>(&self, file: &F) -> Result<Id> {
        let data = serde_json::to_vec(file)?;
        Ok(self.hash_write_full(F::TYPE, &data)?)
    }
}

#[derive(Clone)]
pub struct DecryptBackend<R, C> {
    backend: R,
    key: C,
}

impl<R: ReadBackend, C: CryptoKey> DecryptBackend<R, C> {
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
    fn hash_write_full(&self, tpe: FileType, data: &[u8]) -> Result<Id> {
        let data = self.key().encrypt_data(data)?;
        let id = hash(&data);
        self.write_full(tpe, &id, &mut Cursor::new(data))?;
        Ok(id)
    }
}

impl<R: ReadBackend, C: CryptoKey> DecryptReadBackend for DecryptBackend<R, C> {
    fn read_encrypted_full(&self, tpe: FileType, id: &Id) -> Result<Vec<u8>> {
        Ok(self.key.decrypt_data(&self.backend.read_full(tpe, id)?)?)
    }

    fn read_encrypted_partial(
        &self,
        tpe: FileType,
        id: &Id,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>> {
        Ok(self
            .key
            .decrypt_data(&self.backend.read_partial(tpe, id, offset, length)?)?)
    }
}

impl<R: ReadBackend, C: CryptoKey> ReadBackend for DecryptBackend<R, C> {
    type Error = R::Error;

    fn location(&self) -> &str {
        self.backend.location()
    }

    fn list(&self, tpe: FileType) -> Result<Vec<Id>, Self::Error> {
        self.backend.list(tpe)
    }

    fn list_with_size(&self, tpe: FileType) -> Result<Vec<(Id, u32)>, Self::Error> {
        self.backend.list_with_size(tpe)
    }

    fn read_full(&self, tpe: FileType, id: &Id) -> Result<Vec<u8>, Self::Error> {
        self.backend.read_full(tpe, id)
    }

    fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>, Self::Error> {
        self.backend.read_partial(tpe, id, offset, length)
    }
}

impl<R: WriteBackend, C: CryptoKey> WriteBackend for DecryptBackend<R, C> {
    type Error = R::Error;

    fn write_full(&self, tpe: FileType, id: &Id, r: &mut impl Read) -> Result<(), Self::Error> {
        self.backend.write_full(tpe, id, r)?;
        Ok(())
    }
}
