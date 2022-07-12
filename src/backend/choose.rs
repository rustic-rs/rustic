use std::fs::File;

use anyhow::Result;
use async_trait::async_trait;

use super::{FileType, Id, ReadBackend, WriteBackend};
use super::{LocalBackend, RestBackend};

#[derive(Clone)]
pub enum ChooseBackend {
    Local(LocalBackend),
    Rest(RestBackend),
}

use ChooseBackend::{Local, Rest};

impl ChooseBackend {
    pub fn from_url(url: &str) -> Self {
        if let Some(path) = url.strip_prefix("rest:") {
            return Rest(RestBackend::new(path));
        }
        if let Some(path) = url.strip_prefix("local:") {
            return Local(LocalBackend::new(path));
        }
        Local(LocalBackend::new(url))
    }
}

#[async_trait]
impl ReadBackend for ChooseBackend {
    fn location(&self) -> &str {
        match self {
            Local(local) => local.location(),
            Rest(rest) => rest.location(),
        }
    }

    async fn list_with_size(&self, tpe: FileType) -> Result<Vec<(Id, u32)>> {
        match self {
            Local(local) => local.list_with_size(tpe).await,
            Rest(rest) => rest.list_with_size(tpe).await,
        }
    }

    async fn read_full(&self, tpe: FileType, id: &Id) -> Result<Vec<u8>> {
        match self {
            Local(local) => local.read_full(tpe, id).await,
            Rest(rest) => rest.read_full(tpe, id).await,
        }
    }

    async fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        cacheable: bool,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>> {
        match self {
            Local(local) => local.read_partial(tpe, id, cacheable, offset, length).await,
            Rest(rest) => rest.read_partial(tpe, id, cacheable, offset, length).await,
        }
    }
}

#[async_trait]
impl WriteBackend for ChooseBackend {
    async fn create(&self) -> Result<()> {
        match self {
            Local(local) => local.create().await,
            Rest(rest) => rest.create().await,
        }
    }

    async fn write_file(&self, tpe: FileType, id: &Id, cacheable: bool, f: File) -> Result<()> {
        match self {
            Local(local) => local.write_file(tpe, id, cacheable, f).await,
            Rest(rest) => rest.write_file(tpe, id, cacheable, f).await,
        }
    }

    async fn write_bytes(&self, tpe: FileType, id: &Id, buf: Vec<u8>) -> Result<()> {
        match self {
            Local(local) => local.write_bytes(tpe, id, buf).await,
            Rest(rest) => rest.write_bytes(tpe, id, buf).await,
        }
    }

    async fn remove(&self, tpe: FileType, id: &Id, cacheable: bool) -> Result<()> {
        match self {
            Local(local) => local.remove(tpe, id, cacheable).await,
            Rest(rest) => rest.remove(tpe, id, cacheable).await,
        }
    }
}
