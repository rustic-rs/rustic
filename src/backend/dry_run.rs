use std::num::NonZeroU32;

use anyhow::Result;
use async_trait::async_trait;

use super::{
    DecryptFullBackend, DecryptReadBackend, DecryptWriteBackend, FileType, Id, ReadBackend,
    WriteBackend,
};

#[derive(Clone)]
pub struct DryRunBackend<BE: DecryptFullBackend> {
    be: BE,
    dry_run: bool,
}

impl<BE: DecryptFullBackend> DryRunBackend<BE> {
    pub fn new(be: BE, dry_run: bool) -> Self {
        Self { be, dry_run }
    }
}

#[async_trait]
impl<BE: DecryptFullBackend> DecryptReadBackend for DryRunBackend<BE> {
    async fn read_encrypted_full(&self, tpe: FileType, id: &Id) -> Result<Vec<u8>> {
        self.be.read_encrypted_full(tpe, id).await
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
        self.be
            .read_encrypted_partial(tpe, id, cacheable, offset, length, uncompressed_length)
            .await
    }
}

#[async_trait]
impl<BE: DecryptFullBackend> ReadBackend for DryRunBackend<BE> {
    fn location(&self) -> &str {
        self.be.location()
    }

    async fn list_with_size(&self, tpe: FileType) -> Result<Vec<(Id, u32)>> {
        self.be.list_with_size(tpe).await
    }

    async fn read_full(&self, tpe: FileType, id: &Id) -> Result<Vec<u8>> {
        self.be.read_full(tpe, id).await
    }

    async fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        cacheable: bool,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>> {
        self.be
            .read_partial(tpe, id, cacheable, offset, length)
            .await
    }
}

#[async_trait]
impl<BE: DecryptFullBackend> DecryptWriteBackend for DryRunBackend<BE> {
    type Key = <BE as DecryptWriteBackend>::Key;

    fn key(&self) -> &Self::Key {
        self.be.key()
    }

    async fn hash_write_full(&self, tpe: FileType, data: &[u8]) -> Result<Id> {
        match self.dry_run {
            true => Ok(Id::default()),
            false => self.be.hash_write_full(tpe, data).await,
        }
    }

    fn set_zstd(&mut self, zstd: Option<i32>) {
        match self.dry_run {
            true => {}
            false => self.be.set_zstd(zstd),
        }
    }
}

#[async_trait]
impl<BE: DecryptFullBackend> WriteBackend for DryRunBackend<BE> {
    async fn create(&self) -> Result<()> {
        match self.dry_run {
            true => Ok(()),
            false => self.be.create().await,
        }
    }

    async fn write_bytes(
        &self,
        tpe: FileType,
        id: &Id,
        cacheable: bool,
        buf: Vec<u8>,
    ) -> Result<()> {
        match self.dry_run {
            true => Ok(()),
            false => self.be.write_bytes(tpe, id, cacheable, buf).await,
        }
    }

    async fn remove(&self, tpe: FileType, id: &Id, cacheable: bool) -> Result<()> {
        match self.dry_run {
            true => Ok(()),
            false => self.be.remove(tpe, id, cacheable).await,
        }
    }
}
