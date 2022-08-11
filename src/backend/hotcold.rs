use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;

use super::{FileType, Id, ReadBackend, WriteBackend};

#[derive(Clone)]
pub struct HotColdBackend<BE: WriteBackend> {
    be: BE,
    hot_be: Option<BE>,
}

impl<BE: WriteBackend> HotColdBackend<BE> {
    pub fn new(be: BE, hot_be: Option<BE>) -> Self {
        Self { be, hot_be }
    }
}

#[async_trait]
impl<BE: WriteBackend> ReadBackend for HotColdBackend<BE> {
    fn location(&self) -> &str {
        self.be.location()
    }

    fn set_option(&mut self, option: &str, value: &str) -> Result<()> {
        self.be.set_option(option, value)
    }

    async fn list_with_size(&self, tpe: FileType) -> Result<Vec<(Id, u32)>> {
        self.be.list_with_size(tpe).await
    }

    async fn read_full(&self, tpe: FileType, id: &Id) -> Result<Bytes> {
        match &self.hot_be {
            None => self.be.read_full(tpe, id).await,
            Some(be) => be.read_full(tpe, id).await,
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
        match (&self.hot_be, cacheable || tpe != FileType::Pack) {
            (None, _) | (Some(_), false) => {
                self.be
                    .read_partial(tpe, id, cacheable, offset, length)
                    .await
            }
            (Some(be), true) => be.read_partial(tpe, id, cacheable, offset, length).await,
        }
    }
}

#[async_trait]
impl<BE: WriteBackend> WriteBackend for HotColdBackend<BE> {
    async fn create(&self) -> Result<()> {
        self.be.create().await
    }

    async fn write_bytes(&self, tpe: FileType, id: &Id, cacheable: bool, buf: Bytes) -> Result<()> {
        if let Some(be) = &self.hot_be {
            if tpe != FileType::Config && (cacheable || tpe != FileType::Pack) {
                be.write_bytes(tpe, id, cacheable, buf.clone()).await?;
            }
        }
        self.be.write_bytes(tpe, id, cacheable, buf).await
    }

    async fn remove(&self, tpe: FileType, id: &Id, cacheable: bool) -> Result<()> {
        // First remove cold file
        self.be.remove(tpe, id, cacheable).await?;
        if let Some(be) = &self.hot_be {
            if cacheable || tpe != FileType::Pack {
                be.remove(tpe, id, cacheable).await?;
            }
        }
        Ok(())
    }
}
