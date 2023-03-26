use anyhow::Result;
use bytes::Bytes;

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

impl<BE: DecryptFullBackend> DecryptReadBackend for DryRunBackend<BE> {
    fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        self.be.decrypt(data)
    }
}

impl<BE: DecryptFullBackend> ReadBackend for DryRunBackend<BE> {
    fn location(&self) -> String {
        self.be.location()
    }

    fn set_option(&mut self, option: &str, value: &str) -> Result<()> {
        self.be.set_option(option, value)
    }

    fn list_with_size(&self, tpe: FileType) -> Result<Vec<(Id, u32)>> {
        self.be.list_with_size(tpe)
    }

    fn read_full(&self, tpe: FileType, id: &Id) -> Result<Bytes> {
        self.be.read_full(tpe, id)
    }

    fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        cacheable: bool,
        offset: u32,
        length: u32,
    ) -> Result<Bytes> {
        self.be.read_partial(tpe, id, cacheable, offset, length)
    }
}

impl<BE: DecryptFullBackend> DecryptWriteBackend for DryRunBackend<BE> {
    type Key = <BE as DecryptWriteBackend>::Key;

    fn key(&self) -> &Self::Key {
        self.be.key()
    }

    fn hash_write_full(&self, tpe: FileType, data: &[u8]) -> Result<Id> {
        match self.dry_run {
            true => Ok(Id::default()),
            false => self.be.hash_write_full(tpe, data),
        }
    }

    fn set_zstd(&mut self, zstd: Option<i32>) {
        match self.dry_run {
            true => {}
            false => self.be.set_zstd(zstd),
        }
    }
}

impl<BE: DecryptFullBackend> WriteBackend for DryRunBackend<BE> {
    fn create(&self) -> Result<()> {
        match self.dry_run {
            true => Ok(()),
            false => self.be.create(),
        }
    }

    fn write_bytes(&self, tpe: FileType, id: &Id, cacheable: bool, buf: Bytes) -> Result<()> {
        match self.dry_run {
            true => Ok(()),
            false => self.be.write_bytes(tpe, id, cacheable, buf),
        }
    }

    fn remove(&self, tpe: FileType, id: &Id, cacheable: bool) -> Result<()> {
        match self.dry_run {
            true => Ok(()),
            false => self.be.remove(tpe, id, cacheable),
        }
    }
}
