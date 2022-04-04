use std::fs::File;

use async_trait::async_trait;
use thiserror::Error;

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
        Local(LocalBackend::new(&url))
    }
}

#[derive(Error, Debug)]
#[error(transparent)]
pub struct Error(#[from] anyhow::Error);

#[async_trait]
impl ReadBackend for ChooseBackend {
    type Error = Error;

    fn location(&self) -> &str {
        match self {
            Local(local) => local.location(),
            Rest(rest) => rest.location(),
        }
    }

    async fn list_with_size(&self, tpe: FileType) -> Result<Vec<(Id, u32)>, Self::Error> {
        match self {
            Local(local) => local.list_with_size(tpe).await.map_err(|e| Error(e.into())),
            Rest(rest) => rest.list_with_size(tpe).await.map_err(|e| Error(e.into())),
        }
    }

    async fn read_full(&self, tpe: FileType, id: &Id) -> Result<Vec<u8>, Self::Error> {
        match self {
            Local(local) => local.read_full(tpe, id).await.map_err(|e| Error(e.into())),
            Rest(rest) => rest.read_full(tpe, id).await.map_err(|e| Error(e.into())),
        }
    }

    async fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>, Self::Error> {
        match self {
            Local(local) => local
                .read_partial(tpe, id, offset, length)
                .await
                .map_err(|e| Error(e.into())),
            Rest(rest) => rest
                .read_partial(tpe, id, offset, length)
                .await
                .map_err(|e| Error(e.into())),
        }
    }
}

#[async_trait]
impl WriteBackend for ChooseBackend {
    async fn write_file(&self, tpe: FileType, id: &Id, f: File) -> Result<(), Self::Error> {
        match self {
            Local(local) => local
                .write_file(tpe, id, f)
                .await
                .map_err(|e| Error(e.into())),
            Rest(rest) => rest
                .write_file(tpe, id, f)
                .await
                .map_err(|e| Error(e.into())),
        }
    }

    async fn write_bytes(&self, tpe: FileType, id: &Id, buf: Vec<u8>) -> Result<(), Self::Error> {
        match self {
            Local(local) => local
                .write_bytes(tpe, id, buf)
                .await
                .map_err(|e| Error(e.into())),
            Rest(rest) => rest
                .write_bytes(tpe, id, buf)
                .await
                .map_err(|e| Error(e.into())),
        }
    }
}
