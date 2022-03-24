use std::io::{Cursor, Read};

use anyhow::Result;
use async_trait::async_trait;
use futures::stream::FuturesUnordered;
use indicatif::ProgressBar;
use tokio::{spawn, task::JoinHandle};

use super::{FileType, Id, ReadBackend, RepoFile, WriteBackend};
use crate::crypto::{hash, CryptoKey};

pub trait DecryptFullBackend: DecryptWriteBackend + DecryptReadBackend {}
impl<T: DecryptWriteBackend + DecryptReadBackend> DecryptFullBackend for T {}

#[async_trait]
pub trait DecryptReadBackend: ReadBackend {
    async fn read_encrypted_full(&self, tpe: FileType, id: &Id) -> Result<Vec<u8>>;
    async fn read_encrypted_partial(
        &self,
        tpe: FileType,
        id: &Id,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>>;

    async fn get_file<F: RepoFile>(&self, id: &Id) -> Result<F> {
        let data = self.read_encrypted_full(F::TYPE, id).await?;
        Ok(serde_json::from_slice(&data)?)
    }

    fn stream_all<F: RepoFile>(
        &self,
        p: ProgressBar,
        //    ) -> Result<impl Stream<Item = std::result::Result<F, JoinError>>> {
    ) -> Result<FuturesUnordered<JoinHandle<(Id, F)>>> {
        let list = self.list(F::TYPE)?;
        p.set_length(list.len() as u64);

        let stream: FuturesUnordered<_> = list
            .into_iter()
            .map(|id| {
                let be = self.clone();
                let p = p.clone();
                spawn(async move {
                    let file = be.get_file::<F>(&id).await.unwrap();
                    p.inc(1);
                    (id, file)
                })
            })
            .collect();

        Ok(stream)
    }
}

#[async_trait]
pub trait DecryptWriteBackend: WriteBackend {
    type Key: CryptoKey;
    fn key(&self) -> &Self::Key;
    async fn hash_write_full(&self, tpe: FileType, data: &[u8]) -> Result<Id>;

    async fn save_file<F: RepoFile>(&self, file: &F) -> Result<Id> {
        let data = serde_json::to_vec(file)?;
        Ok(self.hash_write_full(F::TYPE, &data).await?)
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

#[async_trait]
impl<R: WriteBackend, C: CryptoKey> DecryptWriteBackend for DecryptBackend<R, C> {
    type Key = C;
    fn key(&self) -> &Self::Key {
        &self.key
    }
    async fn hash_write_full(&self, tpe: FileType, data: &[u8]) -> Result<Id> {
        let data = self.key().encrypt_data(data)?;
        let id = hash(&data);
        self.write_full(tpe, &id, &mut Cursor::new(data)).await?;
        Ok(id)
    }
}

#[async_trait]
impl<R: ReadBackend, C: CryptoKey> DecryptReadBackend for DecryptBackend<R, C> {
    async fn read_encrypted_full(&self, tpe: FileType, id: &Id) -> Result<Vec<u8>> {
        Ok(self
            .key
            .decrypt_data(&self.backend.read_full(tpe, id).await?)?)
    }

    async fn read_encrypted_partial(
        &self,
        tpe: FileType,
        id: &Id,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>> {
        Ok(self
            .key
            .decrypt_data(&self.backend.read_partial(tpe, id, offset, length).await?)?)
    }
}

#[async_trait]
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

    async fn read_full(&self, tpe: FileType, id: &Id) -> Result<Vec<u8>, Self::Error> {
        self.backend.read_full(tpe, id).await
    }

    async fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>, Self::Error> {
        self.backend.read_partial(tpe, id, offset, length).await
    }
}

#[async_trait]
impl<R: WriteBackend, C: CryptoKey> WriteBackend for DecryptBackend<R, C> {
    type Error = R::Error;

    async fn write_full(
        &self,
        tpe: FileType,
        id: &Id,
        r: &mut (impl Read + Send + Sync),
    ) -> Result<(), Self::Error> {
        self.backend.write_full(tpe, id, r).await?;
        Ok(())
    }
}
