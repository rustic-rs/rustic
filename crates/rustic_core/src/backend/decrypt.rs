use std::num::NonZeroU32;

use bytes::Bytes;
use crossbeam_channel::{unbounded, Receiver};
use indicatif::ProgressBar;
use rayon::prelude::*;
use zstd::stream::{copy_encode, decode_all};

use crate::{
    backend::FileType,
    backend::ReadBackend,
    backend::WriteBackend,
    crypto::{hasher::hash, CryptoKey},
    error::CryptBackendErrorKind,
    id::Id,
    repofile::RepoFile,
    RusticResult,
};

pub trait DecryptFullBackend: DecryptWriteBackend + DecryptReadBackend {}
impl<T: DecryptWriteBackend + DecryptReadBackend> DecryptFullBackend for T {}

pub trait DecryptReadBackend: ReadBackend {
    fn decrypt(&self, data: &[u8]) -> RusticResult<Vec<u8>>;

    fn read_encrypted_full(&self, tpe: FileType, id: &Id) -> RusticResult<Bytes>;

    fn read_encrypted_partial(
        &self,
        tpe: FileType,
        id: &Id,
        cacheable: bool,
        offset: u32,
        length: u32,
        uncompressed_length: Option<NonZeroU32>,
    ) -> RusticResult<Bytes> {
        let mut data = self.decrypt(&self.read_partial(tpe, id, cacheable, offset, length)?)?;
        if let Some(length) = uncompressed_length {
            data = decode_all(&*data)
                .map_err(CryptBackendErrorKind::DecodingZstdCompressedDataFailed)?;
            if data.len() != length.get() as usize {
                return Err(CryptBackendErrorKind::LengthOfUncompressedDataDoesNotMatch.into());
            }
        }
        Ok(data.into())
    }

    fn get_file<F: RepoFile>(&self, id: &Id) -> RusticResult<F> {
        let data = self.read_encrypted_full(F::TYPE, id)?;
        Ok(serde_json::from_slice(&data)
            .map_err(CryptBackendErrorKind::DeserializingFromBytesOfJsonTextFailed)?)
    }

    fn stream_all<F: RepoFile>(
        &self,
        p: ProgressBar,
    ) -> RusticResult<Receiver<RusticResult<(Id, F)>>> {
        let list = self.list(F::TYPE)?;
        self.stream_list(list, p)
    }

    fn stream_list<F: RepoFile>(
        &self,
        list: Vec<Id>,
        p: ProgressBar,
    ) -> RusticResult<Receiver<RusticResult<(Id, F)>>> {
        p.set_length(list.len() as u64);
        let (tx, rx) = unbounded();

        list.into_par_iter()
            .for_each_with((self, p, tx), |(be, p, tx), id| {
                let file = be.get_file::<F>(&id).map(|file| (id, file));
                p.inc(1);
                tx.send(file).unwrap();
            });
        Ok(rx)
    }
}

pub trait DecryptWriteBackend: WriteBackend {
    type Key: CryptoKey;

    fn key(&self) -> &Self::Key;
    fn hash_write_full(&self, tpe: FileType, data: &[u8]) -> RusticResult<Id>;

    fn save_file<F: RepoFile>(&self, file: &F) -> RusticResult<Id> {
        let data = serde_json::to_vec(file)
            .map_err(CryptBackendErrorKind::SerializingToJsonByteVectorFailed)?;
        self.hash_write_full(F::TYPE, &data)
    }

    fn save_list<'a, F: RepoFile, I: ExactSizeIterator<Item = &'a F> + Send>(
        &self,
        list: I,
        p: ProgressBar,
    ) -> RusticResult<()> {
        p.set_length(list.len() as u64);
        list.par_bridge().try_for_each(|file| -> RusticResult<_> {
            _ = self.save_file(file)?;
            p.inc(1);
            Ok(())
        })?;
        p.finish();
        Ok(())
    }

    fn delete_list<'a, I: ExactSizeIterator<Item = &'a Id> + Send>(
        &self,
        tpe: FileType,
        cacheable: bool,
        list: I,
        p: ProgressBar,
    ) -> RusticResult<()> {
        p.set_length(list.len() as u64);
        list.par_bridge().try_for_each(|id| -> RusticResult<_> {
            self.remove(tpe, id, cacheable).unwrap();
            p.inc(1);
            Ok(())
        })?;

        p.finish();
        Ok(())
    }

    fn set_zstd(&mut self, zstd: Option<i32>);
}

#[derive(Clone, Debug)]
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

impl<R: WriteBackend, C: CryptoKey> DecryptWriteBackend for DecryptBackend<R, C> {
    type Key = C;

    fn key(&self) -> &Self::Key {
        &self.key
    }

    fn hash_write_full(&self, tpe: FileType, data: &[u8]) -> RusticResult<Id> {
        let data = match self.zstd {
            Some(level) => {
                let mut out = vec![2_u8];
                copy_encode(data, &mut out, level)
                    .map_err(CryptBackendErrorKind::CopyEncodingDataFailed)?;
                self.key().encrypt_data(&out)?
            }
            None => self.key().encrypt_data(data)?,
        };
        let id = hash(&data);
        self.write_bytes(tpe, &id, false, data.into())?;
        Ok(id)
    }

    fn set_zstd(&mut self, zstd: Option<i32>) {
        self.zstd = zstd;
    }
}

impl<R: ReadBackend, C: CryptoKey> DecryptReadBackend for DecryptBackend<R, C> {
    fn decrypt(&self, data: &[u8]) -> RusticResult<Vec<u8>> {
        self.key.decrypt_data(data)
    }

    fn read_encrypted_full(&self, tpe: FileType, id: &Id) -> RusticResult<Bytes> {
        let decrypted = self.decrypt(&self.read_full(tpe, id)?)?;
        Ok(match decrypted.first() {
            Some(b'{' | b'[') => decrypted, // not compressed
            Some(2) => decode_all(&decrypted[1..])
                .map_err(CryptBackendErrorKind::DecodingZstdCompressedDataFailed)?, // 2 indicates compressed data following
            _ => return Err(CryptBackendErrorKind::DecryptionNotSupportedForBackend)?,
        }
        .into())
    }
}

impl<R: ReadBackend, C: CryptoKey> ReadBackend for DecryptBackend<R, C> {
    fn location(&self) -> String {
        self.backend.location()
    }

    fn set_option(&mut self, option: &str, value: &str) -> RusticResult<()> {
        self.backend.set_option(option, value)
    }

    fn list(&self, tpe: FileType) -> RusticResult<Vec<Id>> {
        self.backend.list(tpe)
    }

    fn list_with_size(&self, tpe: FileType) -> RusticResult<Vec<(Id, u32)>> {
        self.backend.list_with_size(tpe)
    }

    fn read_full(&self, tpe: FileType, id: &Id) -> RusticResult<Bytes> {
        self.backend.read_full(tpe, id)
    }

    fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        cacheable: bool,
        offset: u32,
        length: u32,
    ) -> RusticResult<Bytes> {
        self.backend
            .read_partial(tpe, id, cacheable, offset, length)
    }
}

impl<R: WriteBackend, C: CryptoKey> WriteBackend for DecryptBackend<R, C> {
    fn create(&self) -> RusticResult<()> {
        self.backend.create()
    }

    fn write_bytes(&self, tpe: FileType, id: &Id, cacheable: bool, buf: Bytes) -> RusticResult<()> {
        self.backend.write_bytes(tpe, id, cacheable, buf)
    }

    fn remove(&self, tpe: FileType, id: &Id, cacheable: bool) -> RusticResult<()> {
        self.backend.remove(tpe, id, cacheable)
    }
}
