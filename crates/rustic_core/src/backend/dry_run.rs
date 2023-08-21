use bytes::Bytes;
use zstd::decode_all;

use crate::{
    backend::{
        decrypt::DecryptFullBackend, decrypt::DecryptReadBackend, decrypt::DecryptWriteBackend,
        FileType, Id, ReadBackend, WriteBackend,
    },
    error::CryptBackendErrorKind,
    RusticResult,
};

#[derive(Clone, Debug)]
pub struct DryRunBackend<BE: DecryptFullBackend> {
    be: BE,
    dry_run: bool,
}

impl<BE: DecryptFullBackend> DryRunBackend<BE> {
    pub const fn new(be: BE, dry_run: bool) -> Self {
        Self { be, dry_run }
    }
}

impl<BE: DecryptFullBackend> DecryptReadBackend for DryRunBackend<BE> {
    fn decrypt(&self, data: &[u8]) -> RusticResult<Vec<u8>> {
        self.be.decrypt(data)
    }

    fn read_encrypted_full(&self, tpe: FileType, id: &Id) -> RusticResult<Bytes> {
        let decrypted = self.decrypt(&self.read_full(tpe, id)?)?;
        Ok(match decrypted.first() {
            Some(b'{' | b'[') => decrypted, // not compressed
            Some(2) => decode_all(&decrypted[1..])
                .map_err(CryptBackendErrorKind::DecodingZstdCompressedDataFailed)?, // 2 indicates compressed data following
            _ => return Err(CryptBackendErrorKind::DecryptionNotSupportedForBackend.into()),
        }
        .into())
    }
}

impl<BE: DecryptFullBackend> ReadBackend for DryRunBackend<BE> {
    fn location(&self) -> String {
        self.be.location()
    }

    fn set_option(&mut self, option: &str, value: &str) -> RusticResult<()> {
        self.be.set_option(option, value)
    }

    fn list_with_size(&self, tpe: FileType) -> RusticResult<Vec<(Id, u32)>> {
        self.be.list_with_size(tpe)
    }

    fn read_full(&self, tpe: FileType, id: &Id) -> RusticResult<Bytes> {
        self.be.read_full(tpe, id)
    }

    fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        cacheable: bool,
        offset: u32,
        length: u32,
    ) -> RusticResult<Bytes> {
        self.be.read_partial(tpe, id, cacheable, offset, length)
    }
}

impl<BE: DecryptFullBackend> DecryptWriteBackend for DryRunBackend<BE> {
    type Key = <BE as DecryptWriteBackend>::Key;

    fn key(&self) -> &Self::Key {
        self.be.key()
    }

    fn hash_write_full(&self, tpe: FileType, data: &[u8]) -> RusticResult<Id> {
        if self.dry_run {
            Ok(Id::default())
        } else {
            self.be.hash_write_full(tpe, data)
        }
    }

    fn set_zstd(&mut self, zstd: Option<i32>) {
        if !self.dry_run {
            self.be.set_zstd(zstd);
        }
    }
}

impl<BE: DecryptFullBackend> WriteBackend for DryRunBackend<BE> {
    fn create(&self) -> RusticResult<()> {
        if self.dry_run {
            Ok(())
        } else {
            self.be.create()
        }
    }

    fn write_bytes(&self, tpe: FileType, id: &Id, cacheable: bool, buf: Bytes) -> RusticResult<()> {
        if self.dry_run {
            Ok(())
        } else {
            self.be.write_bytes(tpe, id, cacheable, buf)
        }
    }

    fn remove(&self, tpe: FileType, id: &Id, cacheable: bool) -> RusticResult<()> {
        if self.dry_run {
            Ok(())
        } else {
            self.be.remove(tpe, id, cacheable)
        }
    }
}
