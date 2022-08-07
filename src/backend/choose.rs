use anyhow::{bail, Result};
use async_trait::async_trait;
use bytes::Bytes;

use super::{FileType, Id, ReadBackend, WriteBackend};
use super::{LocalBackend, RcloneBackend, RestBackend};

#[derive(Clone)]
pub enum ChooseBackend {
    Local(LocalBackend),
    Rest(RestBackend),
    Rclone(RcloneBackend),
}

use ChooseBackend::{Local, Rclone, Rest};

impl ChooseBackend {
    pub fn from_url(url: &str) -> Result<Self> {
        Ok(match url.split_once(':') {
            Some(("rclone", path)) => Rclone(RcloneBackend::new(path)?),
            Some(("rest", path)) => Rest(RestBackend::new(path)),
            Some(("local", path)) => Local(LocalBackend::new(path)),
            Some((backend, _)) => bail!("backend {backend} is not supported!"),
            None => Local(LocalBackend::new(url)),
        })
    }
}

#[async_trait]
impl ReadBackend for ChooseBackend {
    fn location(&self) -> &str {
        match self {
            Local(local) => local.location(),
            Rest(rest) => rest.location(),
            Rclone(rclone) => rclone.location(),
        }
    }

    async fn list_with_size(&self, tpe: FileType) -> Result<Vec<(Id, u32)>> {
        match self {
            Local(local) => local.list_with_size(tpe).await,
            Rest(rest) => rest.list_with_size(tpe).await,
            Rclone(rclone) => rclone.list_with_size(tpe).await,
        }
    }

    async fn read_full(&self, tpe: FileType, id: &Id) -> Result<Bytes> {
        match self {
            Local(local) => local.read_full(tpe, id).await,
            Rest(rest) => rest.read_full(tpe, id).await,
            Rclone(rclone) => rclone.read_full(tpe, id).await,
        }
    }

    async fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        cacheable: bool,
        offset: u32,
        length: u32,
    ) -> Result<Bytes> {
        match self {
            Local(local) => local.read_partial(tpe, id, cacheable, offset, length).await,
            Rest(rest) => rest.read_partial(tpe, id, cacheable, offset, length).await,
            Rclone(rclone) => {
                rclone
                    .read_partial(tpe, id, cacheable, offset, length)
                    .await
            }
        }
    }
}

#[async_trait]
impl WriteBackend for ChooseBackend {
    async fn create(&self) -> Result<()> {
        match self {
            Local(local) => local.create().await,
            Rest(rest) => rest.create().await,
            Rclone(rclone) => rclone.create().await,
        }
    }

    async fn write_bytes(&self, tpe: FileType, id: &Id, cacheable: bool, buf: Bytes) -> Result<()> {
        match self {
            Local(local) => local.write_bytes(tpe, id, cacheable, buf).await,
            Rest(rest) => rest.write_bytes(tpe, id, cacheable, buf).await,
            Rclone(rclone) => rclone.write_bytes(tpe, id, cacheable, buf).await,
        }
    }

    async fn remove(&self, tpe: FileType, id: &Id, cacheable: bool) -> Result<()> {
        match self {
            Local(local) => local.remove(tpe, id, cacheable).await,
            Rest(rest) => rest.remove(tpe, id, cacheable).await,
            Rclone(rclone) => rclone.remove(tpe, id, cacheable).await,
        }
    }
}
