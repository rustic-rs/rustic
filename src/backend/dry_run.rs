use std::io::Read;

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
    fn read_encrypted_full(&self, tpe: FileType, id: &Id) -> Result<Vec<u8>, Self::Error> {
        self.be.read_encrypted_full(tpe, id)
    }
    fn read_encrypted_partial(
        &self,
        tpe: FileType,
        id: &Id,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>, Self::Error> {
        self.be.read_encrypted_partial(tpe, id, offset, length)
    }
}

impl<BE: DecryptFullBackend> ReadBackend for DryRunBackend<BE> {
    type Error = <BE as ReadBackend>::Error;
    fn location(&self) -> &str {
        self.be.location()
    }

    fn list_with_size(&self, tpe: FileType) -> Result<Vec<(Id, u32)>, Self::Error> {
        self.be.list_with_size(tpe)
    }

    fn read_full(&self, tpe: FileType, id: &Id) -> Result<Vec<u8>, Self::Error> {
        self.be.read_full(tpe, id)
    }

    fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>, Self::Error> {
        self.be.read_partial(tpe, id, offset, length)
    }
}

impl<BE: DecryptFullBackend> DecryptWriteBackend for DryRunBackend<BE> {
    type Key = <BE as DecryptWriteBackend>::Key;

    fn key(&self) -> &Self::Key {
        self.be.key()
    }

    fn hash_write_full(&self, tpe: FileType, data: &[u8]) -> Result<Id, Self::Error> {
        match self.dry_run {
            true => Ok(Id::default()),
            false => self.be.hash_write_full(tpe, data),
        }
    }
}

impl<BE: DecryptFullBackend> WriteBackend for DryRunBackend<BE> {
    type Error = <BE as WriteBackend>::Error;
    fn write_full(&self, tpe: FileType, id: &Id, r: &mut impl Read) -> Result<(), Self::Error> {
        match self.dry_run {
            true => Ok(()),
            false => self.be.write_full(tpe, id, r),
        }
    }
}
