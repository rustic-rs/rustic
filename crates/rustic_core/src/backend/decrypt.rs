use std::num::NonZeroU32;

use bytes::Bytes;
use crossbeam_channel::{unbounded, Receiver};
use rayon::prelude::*;
use zstd::stream::{copy_encode, decode_all};

pub use zstd::compression_level_range;

/// The maximum compression level allowed by zstd
pub fn max_compression_level() -> i32 {
    *compression_level_range().end()
}

use crate::{
    backend::FileType,
    backend::ReadBackend,
    backend::WriteBackend,
    crypto::{hasher::hash, CryptoKey},
    error::CryptBackendErrorKind,
    id::Id,
    repofile::RepoFile,
    Progress, RusticResult,
};

/// A backend that can decrypt data.
/// This is a trait that is implemented by all backends that can decrypt data.
/// It is implemented for all backends that implement `DecryptWriteBackend` and `DecryptReadBackend`.
/// This trait is used by the `Repository` to decrypt data.
pub trait DecryptFullBackend: DecryptWriteBackend + DecryptReadBackend {}

impl<T: DecryptWriteBackend + DecryptReadBackend> DecryptFullBackend for T {}

pub trait DecryptReadBackend: ReadBackend {
    /// Decrypts the given data.
    ///
    /// # Arguments
    ///
    /// * `data` - The data to decrypt.
    ///
    /// # Errors
    ///
    /// If the data could not be decrypted.
    fn decrypt(&self, data: &[u8]) -> RusticResult<Vec<u8>>;

    /// Reads the given file.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `id` - The id of the file.
    ///
    /// # Errors
    ///
    /// If the file could not be read.
    fn read_encrypted_full(&self, tpe: FileType, id: &Id) -> RusticResult<Bytes>;

    /// Reads the given file from partial data.
    ///
    /// # Arguments
    ///
    /// * `data` - The partial data to decrypt.
    /// * `uncompressed_length` - The length of the uncompressed data.
    ///
    /// # Errors
    ///
    /// * [`CryptBackendErrorKind::DecodingZstdCompressedDataFailed`] - If the data could not be decoded.
    /// * [`CryptBackendErrorKind::LengthOfUncompressedDataDoesNotMatch`] - If the length of the uncompressed data does not match the given length.
    ///
    /// [`CryptBackendErrorKind::DecodingZstdCompressedDataFailed`]: crate::error::CryptBackendErrorKind::DecodingZstdCompressedDataFailed
    /// [`CryptBackendErrorKind::LengthOfUncompressedDataDoesNotMatch`]: crate::error::CryptBackendErrorKind::LengthOfUncompressedDataDoesNotMatch
    fn read_encrypted_from_partial(
        &self,
        data: &[u8],
        uncompressed_length: Option<NonZeroU32>,
    ) -> RusticResult<Bytes> {
        let mut data = self.decrypt(data)?;
        if let Some(length) = uncompressed_length {
            data = decode_all(&*data)
                .map_err(CryptBackendErrorKind::DecodingZstdCompressedDataFailed)?;
            if data.len() != length.get() as usize {
                return Err(CryptBackendErrorKind::LengthOfUncompressedDataDoesNotMatch.into());
            }
        }
        Ok(data.into())
    }

    /// Reads the given file with the given offset and length.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `id` - The id of the file.
    /// * `cacheable` - Whether the file should be cached.
    /// * `offset` - The offset to read from.
    /// * `length` - The length to read.
    /// * `uncompressed_length` - The length of the uncompressed data.
    ///
    /// # Errors
    ///
    /// If the file could not be read.
    fn read_encrypted_partial(
        &self,
        tpe: FileType,
        id: &Id,
        cacheable: bool,
        offset: u32,
        length: u32,
        uncompressed_length: Option<NonZeroU32>,
    ) -> RusticResult<Bytes> {
        self.read_encrypted_from_partial(
            &self.read_partial(tpe, id, cacheable, offset, length)?,
            uncompressed_length,
        )
    }

    /// Gets the given file.
    ///
    /// # Arguments
    ///
    /// * `id` - The id of the file.
    ///
    /// # Errors
    ///
    /// If the file could not be read.
    fn get_file<F: RepoFile>(&self, id: &Id) -> RusticResult<F> {
        let data = self.read_encrypted_full(F::TYPE, id)?;
        Ok(serde_json::from_slice(&data)
            .map_err(CryptBackendErrorKind::DeserializingFromBytesOfJsonTextFailed)?)
    }

    /// Streams all files.
    ///
    /// # Arguments
    ///
    /// * `p` - The progress bar.
    ///
    /// # Errors
    ///
    /// If the files could not be read.
    fn stream_all<F: RepoFile>(
        &self,
        p: &impl Progress,
    ) -> RusticResult<Receiver<RusticResult<(Id, F)>>> {
        let list = self.list(F::TYPE)?;
        self.stream_list(list, p)
    }

    /// Streams a list of files.
    ///
    /// # Arguments
    ///
    /// * `list` - The list of files to stream.
    /// * `p` - The progress bar.
    ///
    /// # Errors
    ///
    /// If the files could not be read.
    fn stream_list<F: RepoFile>(
        &self,
        list: Vec<Id>,
        p: &impl Progress,
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
    /// The type of the key.
    type Key: CryptoKey;

    /// Gets the key.
    fn key(&self) -> &Self::Key;

    /// Writes the given data to the backend and returns the id of the data.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `data` - The data to write.
    ///
    /// # Errors
    ///
    /// If the data could not be written.
    ///
    /// # Returns
    ///
    /// The id of the data. (TODO: Check if this is correct)
    fn hash_write_full(&self, tpe: FileType, data: &[u8]) -> RusticResult<Id>;

    /// Saves the given file.
    ///
    /// # Arguments
    ///
    /// * `file` - The file to save.
    ///
    /// # Errors
    ///
    /// * [`CryptBackendErrorKind::SerializingToJsonByteVectorFailed`] - If the file could not be serialized to json.
    ///
    /// # Returns
    ///
    /// The id of the file.
    ///
    /// [`CryptBackendErrorKind::SerializingToJsonByteVectorFailed`]: crate::error::CryptBackendErrorKind::SerializingToJsonByteVectorFailed
    fn save_file<F: RepoFile>(&self, file: &F) -> RusticResult<Id> {
        let data = serde_json::to_vec(file)
            .map_err(CryptBackendErrorKind::SerializingToJsonByteVectorFailed)?;
        self.hash_write_full(F::TYPE, &data)
    }

    /// Saves the given list of files.
    ///
    /// # Arguments
    ///
    /// * `list` - The list of files to save.
    /// * `p` - The progress bar.
    ///
    /// # Errors
    ///
    /// * [`CryptBackendErrorKind::SerializingToJsonByteVectorFailed`] - If the file could not be serialized to json.
    fn save_list<'a, F: RepoFile, I: ExactSizeIterator<Item = &'a F> + Send>(
        &self,
        list: I,
        p: impl Progress,
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

    /// Deletes the given list of files.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the files.
    /// * `cacheable` - Whether the files should be cached.
    /// * `list` - The list of files to delete.
    /// * `p` - The progress bar.
    ///
    /// # Panics
    ///
    /// If the files could not be deleted.
    fn delete_list<'a, I: ExactSizeIterator<Item = &'a Id> + Send>(
        &self,
        tpe: FileType,
        cacheable: bool,
        list: I,
        p: impl Progress,
    ) -> RusticResult<()> {
        p.set_length(list.len() as u64);
        list.par_bridge().try_for_each(|id| -> RusticResult<_> {
            // TODO: Don't panic on file not being able to be deleted.
            self.remove(tpe, id, cacheable).unwrap();
            p.inc(1);
            Ok(())
        })?;

        p.finish();
        Ok(())
    }

    fn set_zstd(&mut self, zstd: Option<i32>);
}

/// A backend that can decrypt data.
///
/// # Type Parameters
///
/// * `R` - The type of the backend to decrypt.
/// * `C` - The type of the key to decrypt the backend with.
#[derive(Clone, Debug)]
pub struct DecryptBackend<R, C> {
    /// The backend to decrypt.
    backend: R,
    /// The key to decrypt the backend with.
    key: C,
    /// The compression level to use for zstd.
    zstd: Option<i32>,
}

impl<R: ReadBackend, C: CryptoKey> DecryptBackend<R, C> {
    /// Creates a new decrypt backend.
    ///
    /// # Type Parameters
    ///
    /// * `R` - The type of the backend to decrypt.
    /// * `C` - The type of the key to decrypt the backend with.
    ///
    /// # Arguments
    ///
    /// * `be` - The backend to decrypt.
    /// * `key` - The key to decrypt the backend with.
    ///
    /// # Returns
    ///
    /// The new decrypt backend.
    pub fn new(be: &R, key: C) -> Self {
        Self {
            backend: be.clone(),
            key,
            zstd: None,
        }
    }
}

impl<R: WriteBackend, C: CryptoKey> DecryptWriteBackend for DecryptBackend<R, C> {
    /// The type of the key.
    type Key = C;

    /// Gets the key.
    fn key(&self) -> &Self::Key {
        &self.key
    }

    /// Writes the given data to the backend and returns the id of the data.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `data` - The data to write.
    ///
    /// # Errors
    ///
    /// * [`CryptBackendErrorKind::CopyEncodingDataFailed`] - If the data could not be encoded.
    ///
    /// # Returns
    ///
    /// The id of the data.
    ///
    /// [`CryptBackendErrorKind::CopyEncodingDataFailed`]: crate::error::CryptBackendErrorKind::CopyEncodingDataFailed
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

    /// Sets the compression level to use for zstd.
    ///
    /// # Arguments
    ///
    /// * `zstd` - The compression level to use for zstd.
    fn set_zstd(&mut self, zstd: Option<i32>) {
        self.zstd = zstd;
    }
}

impl<R: ReadBackend, C: CryptoKey> DecryptReadBackend for DecryptBackend<R, C> {
    /// Decrypts the given data.
    ///
    /// # Arguments
    ///
    /// * `data` - The data to decrypt.
    ///
    /// # Returns
    ///
    /// A vector containing the decrypted data.
    fn decrypt(&self, data: &[u8]) -> RusticResult<Vec<u8>> {
        self.key.decrypt_data(data)
    }

    /// Reads encrypted data from the backend.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `id` - The id of the file.
    ///
    /// # Errors
    ///
    /// * [`CryptBackendErrorKind::DecryptionNotSupportedForBackend`] - If the backend does not support decryption.
    /// * [`CryptBackendErrorKind::DecodingZstdCompressedDataFailed`] - If the data could not be decoded.
    ///
    /// [`CryptBackendErrorKind::DecryptionNotSupportedForBackend`]: crate::error::CryptBackendErrorKind::DecryptionNotSupportedForBackend
    /// [`CryptBackendErrorKind::DecodingZstdCompressedDataFailed`]: crate::error::CryptBackendErrorKind::DecodingZstdCompressedDataFailed
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
