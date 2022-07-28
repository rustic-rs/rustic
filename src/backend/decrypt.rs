use std::fs::File;
use std::num::NonZeroU32;

use anyhow::{bail, Result};
use async_trait::async_trait;
use futures::{stream, stream::FuturesUnordered, StreamExt};
use indicatif::ProgressBar;
use tokio::{spawn, task::JoinHandle};
use zstd::stream::{copy_encode, decode_all};

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
        cacheable: bool,
        offset: u32,
        length: u32,
        uncompressed_length: Option<NonZeroU32>,
    ) -> Result<Vec<u8>>;

    async fn get_file<F: RepoFile>(&self, id: &Id) -> Result<F> {
        let data = self.read_encrypted_full(F::TYPE, id).await?;
        Ok(serde_json::from_slice(&data)?)
    }

    async fn stream_all<F: RepoFile>(
        &self,
        p: ProgressBar,
    ) -> Result<FuturesUnordered<JoinHandle<(Id, F)>>> {
        let list = self.list(F::TYPE).await?;
        self.stream_list(list, p).await
    }

    async fn stream_list<F: RepoFile>(
        &self,
        list: Vec<Id>,
        p: ProgressBar,
    ) -> Result<FuturesUnordered<JoinHandle<(Id, F)>>> {
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

    async fn save_list<F: RepoFile>(&self, list: Vec<F>, p: ProgressBar) -> Result<()> {
        p.set_length(list.len() as u64);
        stream::iter(list.into_iter().map(|file| {
            let be = self.clone();
            let p = p.clone();
            (file, be, p)
        }))
        .for_each_concurrent(5, |(file, be, p)| async move {
            be.save_file(&file).await.unwrap();
            p.inc(1);
        })
        .await;
        p.finish();
        Ok(())
    }

    async fn delete_list(
        &self,
        tpe: FileType,
        cacheable: bool,
        list: Vec<Id>,
        p: ProgressBar,
    ) -> Result<()> {
        p.set_length(list.len() as u64);
        stream::iter(list.into_iter().map(|id| {
            let be = self.clone();
            let p = p.clone();
            (id, be, p)
        }))
        .for_each_concurrent(20, |(id, be, p)| async move {
            be.remove(tpe, &id, cacheable).await.unwrap();
            p.inc(1);
        })
        .await;

        p.finish();
        Ok(())
    }

    fn set_zstd(&mut self, zstd: Option<i32>);
}

#[derive(Clone)]
pub struct DecryptBackend<R, C> {
    backend: R,
    key: C,
    zstd: Option<i32>,
}

impl<R: ReadBackend, C: CryptoKey> DecryptBackend<R, C> {
    pub fn new(be: &R, key: C) -> Self {
        Self {
            backend: be.clone(),
            key,
            zstd: None,
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
        let data = match self.zstd {
            Some(level) => {
                let mut out = vec![2_u8];
                copy_encode(data, &mut out, level)?;
                self.key().encrypt_data(&out)?
            }
            None => self.key().encrypt_data(data)?,
        };
        let id = hash(&data);
        self.write_bytes(tpe, &id, data).await?;
        Ok(id)
    }

    fn set_zstd(&mut self, zstd: Option<i32>) {
        self.zstd = zstd;
    }
}

#[async_trait]
impl<R: ReadBackend, C: CryptoKey> DecryptReadBackend for DecryptBackend<R, C> {
    async fn read_encrypted_full(&self, tpe: FileType, id: &Id) -> Result<Vec<u8>> {
        let decrypted = self
            .key
            .decrypt_data(&self.backend.read_full(tpe, id).await?)?;
        Ok(match decrypted[0] {
            b'{' | b'[' => decrypted,          // not compressed
            2 => decode_all(&decrypted[1..])?, // 2 indicates compressed data following
            _ => bail!("not supported"),
        })
    }

    async fn read_encrypted_partial(
        &self,
        tpe: FileType,
        id: &Id,
        cacheable: bool,
        offset: u32,
        length: u32,
        uncompressed_length: Option<NonZeroU32>,
    ) -> Result<Vec<u8>> {
        let mut data = self.key.decrypt_data(
            &self
                .backend
                .read_partial(tpe, id, cacheable, offset, length)
                .await?,
        )?;
        if let Some(length) = uncompressed_length {
            data = decode_all(&*data).unwrap();
            if data.len() != length.get() as usize {
                bail!("length of uncompressed data does not match!");
            }
        }
        Ok(data)
    }
}

#[async_trait]
impl<R: ReadBackend, C: CryptoKey> ReadBackend for DecryptBackend<R, C> {
    fn location(&self) -> &str {
        self.backend.location()
    }

    async fn list(&self, tpe: FileType) -> Result<Vec<Id>> {
        self.backend.list(tpe).await
    }

    async fn list_with_size(&self, tpe: FileType) -> Result<Vec<(Id, u32)>> {
        self.backend.list_with_size(tpe).await
    }

    async fn read_full(&self, tpe: FileType, id: &Id) -> Result<Vec<u8>> {
        self.backend.read_full(tpe, id).await
    }

    async fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        cacheable: bool,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>> {
        self.backend
            .read_partial(tpe, id, cacheable, offset, length)
            .await
    }
}

#[async_trait]
impl<R: WriteBackend, C: CryptoKey> WriteBackend for DecryptBackend<R, C> {
    async fn create(&self) -> Result<()> {
        self.backend.create().await
    }

    async fn write_file(&self, tpe: FileType, id: &Id, cacheable: bool, f: File) -> Result<()> {
        self.backend.write_file(tpe, id, cacheable, f).await
    }

    async fn write_bytes(&self, tpe: FileType, id: &Id, buf: Vec<u8>) -> Result<()> {
        self.backend.write_bytes(tpe, id, buf).await
    }

    async fn remove(&self, tpe: FileType, id: &Id, cacheable: bool) -> Result<()> {
        self.backend.remove(tpe, id, cacheable).await
    }
}
