use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
};

use bytes::Bytes;
use dirs::cache_dir;
use log::{trace, warn};
use walkdir::WalkDir;

use crate::{
    backend::{FileType, ReadBackend, WriteBackend},
    error::CacheBackendErrorKind,
    error::RusticResult,
    id::Id,
};

/// Backend that caches data.
///
/// This backend caches data in a directory.
/// It can be used to cache data from a remote backend.
///
/// # Type Parameters
///
/// * `BE` - The backend to cache.
#[derive(Clone, Debug)]
pub struct CachedBackend<BE: WriteBackend> {
    /// The backend to cache.
    be: BE,
    /// The cache.
    cache: Option<Cache>,
}

impl<BE: WriteBackend> CachedBackend<BE> {
    /// Create a new [`CachedBackend`] from a given backend.
    ///
    /// # Type Parameters
    ///
    /// * `BE` - The backend to cache.
    pub fn new(be: BE, cache: Option<Cache>) -> Self {
        Self { be, cache }
    }
}

impl<BE: WriteBackend> ReadBackend for CachedBackend<BE> {
    /// Returns the location of the backend as a String.
    fn location(&self) -> String {
        self.be.location()
    }

    /// Sets an option of the backend.
    ///
    ///  # Arguments
    ///
    /// * `option` - The option to set.
    /// * `value` - The value to set the option to.
    fn set_option(&mut self, option: &str, value: &str) -> RusticResult<()> {
        self.be.set_option(option, value)
    }

    /// Lists all files with their size of the given type.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the files to list.
    ///
    /// # Errors
    ///
    /// If the backend does not support listing files.
    ///
    /// # Returns
    ///
    /// A vector of tuples containing the id and size of the files.
    fn list_with_size(&self, tpe: FileType) -> RusticResult<Vec<(Id, u32)>> {
        let list = self.be.list_with_size(tpe)?;

        if let Some(cache) = &self.cache {
            if tpe.is_cacheable() {
                cache.remove_not_in_list(tpe, &list)?;
            }
        }

        Ok(list)
    }

    /// Reads full data of the given file.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `id` - The id of the file.
    ///
    /// # Errors
    ///
    /// * [`CacheBackendErrorKind::FromIoError`] - If the file could not be read.
    ///
    /// # Returns
    ///
    /// The data read.
    ///
    /// [`CacheBackendErrorKind::FromIoError`]: crate::error::CacheBackendErrorKind::FromIoError
    fn read_full(&self, tpe: FileType, id: &Id) -> RusticResult<Bytes> {
        match (&self.cache, tpe.is_cacheable()) {
            (None, _) | (Some(_), false) => self.be.read_full(tpe, id),
            (Some(cache), true) => cache.read_full(tpe, id).map_or_else(
                |err| {
                    warn!("Error in cache backend: {err}");
                    let res = self.be.read_full(tpe, id);
                    if let Ok(data) = &res {
                        _ = cache.write_bytes(tpe, id, data.clone());
                    }
                    res
                },
                Ok,
            ),
        }
    }

    /// Reads partial data of the given file.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `id` - The id of the file.
    /// * `cacheable` - Whether the file is cacheable.
    /// * `offset` - The offset to read from.
    /// * `length` - The length to read.
    ///
    /// # Errors
    ///
    /// * [`CacheBackendErrorKind::FromIoError`] - If the file could not be read.
    ///
    /// # Returns
    ///
    /// The data read.
    ///
    /// [`CacheBackendErrorKind::FromIoError`]: crate::error::CacheBackendErrorKind::FromIoError
    fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        cacheable: bool,
        offset: u32,
        length: u32,
    ) -> RusticResult<Bytes> {
        match (&self.cache, cacheable || tpe.is_cacheable()) {
            (None, _) | (Some(_), false) => {
                self.be.read_partial(tpe, id, cacheable, offset, length)
            }
            (Some(cache), true) => {
                cache.read_partial(tpe, id, offset, length).map_or_else(
                    |err| {
                        warn!("Error in cache backend: {err}");
                        match self.be.read_full(tpe, id) {
                            // read full file, save to cache and return partial content from cache
                            // TODO: - Do not read to memory, but use a Reader
                            //       - Don't read from cache, but use the right part of the read content
                            Ok(data) => {
                                if cache.write_bytes(tpe, id, data).is_ok() {
                                    cache.read_partial(tpe, id, offset, length)
                                } else {
                                    self.be.read_partial(tpe, id, false, offset, length)
                                }
                            }
                            error => error,
                        }
                    },
                    Ok,
                )
            }
        }
    }
}

impl<BE: WriteBackend> WriteBackend for CachedBackend<BE> {
    /// Creates the backend.
    fn create(&self) -> RusticResult<()> {
        self.be.create()
    }

    /// Writes the given data to the given file.
    ///
    /// If the file is cacheable, it will also be written to the cache.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `id` - The id of the file.
    /// * `cacheable` - Whether the file is cacheable.
    /// * `buf` - The data to write.
    fn write_bytes(&self, tpe: FileType, id: &Id, cacheable: bool, buf: Bytes) -> RusticResult<()> {
        if let Some(cache) = &self.cache {
            if cacheable || tpe.is_cacheable() {
                _ = cache.write_bytes(tpe, id, buf.clone());
            }
        }
        self.be.write_bytes(tpe, id, cacheable, buf)
    }

    /// Removes the given file.
    ///
    /// If the file is cacheable, it will also be removed from the cache.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `id` - The id of the file.
    fn remove(&self, tpe: FileType, id: &Id, cacheable: bool) -> RusticResult<()> {
        if let Some(cache) = &self.cache {
            if cacheable || tpe.is_cacheable() {
                _ = cache.remove(tpe, id);
            }
        }
        self.be.remove(tpe, id, cacheable)
    }
}

/// Backend that caches data in a directory.
#[derive(Clone, Debug)]
pub struct Cache {
    /// The path to the cache.
    path: PathBuf,
}

impl Cache {
    /// Creates a new [`Cache`] with the given id.
    ///
    /// If no path is given, the cache will be created in the default cache directory.
    ///
    /// # Arguments
    ///
    /// * `id` - The id of the cache.
    /// * `path` - The path to the cache.
    ///
    /// # Errors
    ///
    /// * [`CacheBackendErrorKind::NoCacheDirectory`] - If no path is given and the default cache directory could not be determined.
    /// * [`CacheBackendErrorKind::FromIoError`] - If the cache directory could not be created.
    ///
    /// [`CacheBackendErrorKind::NoCacheDirectory`]: crate::error::CacheBackendErrorKind::NoCacheDirectory
    /// [`CacheBackendErrorKind::FromIoError`]: crate::error::CacheBackendErrorKind::FromIoError
    pub fn new(id: Id, path: Option<PathBuf>) -> RusticResult<Self> {
        let mut path = path.unwrap_or({
            let mut dir = cache_dir().ok_or_else(|| CacheBackendErrorKind::NoCacheDirectory)?;
            dir.push("rustic");
            dir
        });
        fs::create_dir_all(&path).map_err(CacheBackendErrorKind::FromIoError)?;
        cachedir::ensure_tag(&path).map_err(CacheBackendErrorKind::FromIoError)?;
        path.push(id.to_hex());
        fs::create_dir_all(&path).map_err(CacheBackendErrorKind::FromIoError)?;
        Ok(Self { path })
    }

    /// Returns the path to the location of this [`Cache`].
    ///
    /// # Panics
    ///
    /// Panics if the path is not valid unicode.
    // TODO: Does this need to panic? Result?
    #[must_use]
    pub fn location(&self) -> &str {
        self.path.to_str().unwrap()
    }

    /// Returns the path to the directory of the given type.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the directory.
    /// * `id` - The id of the directory.
    #[must_use]
    pub fn dir(&self, tpe: FileType, id: &Id) -> PathBuf {
        let hex_id = id.to_hex();
        self.path.join(tpe.dirname()).join(&hex_id[0..2])
    }

    /// Returns the path to the given file.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `id` - The id of the file.
    #[must_use]
    pub fn path(&self, tpe: FileType, id: &Id) -> PathBuf {
        let hex_id = id.to_hex();
        self.path
            .join(tpe.dirname())
            .join(&hex_id[0..2])
            .join(hex_id)
    }

    /// Lists all files with their size of the given type.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the files to list.
    ///
    /// # Errors
    ///
    /// * [`CacheBackendErrorKind::FromIoError`] - If the cache directory could not be read.
    /// * [`IdErrorKind::HexError`] - If the string is not a valid hexadecimal string
    ///
    /// [`CacheBackendErrorKind::FromIoError`]: crate::error::CacheBackendErrorKind::FromIoError
    /// [`IdErrorKind::HexError`]: crate::error::IdErrorKind::HexError
    pub fn list_with_size(&self, tpe: FileType) -> RusticResult<HashMap<Id, u32>> {
        let path = self.path.join(tpe.dirname());

        let walker = WalkDir::new(path)
            .into_iter()
            .filter_map(walkdir::Result::ok)
            .filter(|e| {
                // only use files with length of 64 which are valid hex
                e.file_type().is_file()
                    && e.file_name().len() == 64
                    && e.file_name().is_ascii()
                    && e.file_name()
                        .to_str()
                        .unwrap()
                        .chars()
                        .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
            })
            .map(|e| {
                (
                    Id::from_hex(e.file_name().to_str().unwrap()).unwrap(),
                    // handle errors in metadata by returning a size of 0
                    e.metadata().map_or(0, |m| m.len().try_into().unwrap_or(0)),
                )
            });

        Ok(walker.collect())
    }

    /// Removes all files from the cache that are not in the given list.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the files.
    /// * `list` - The list of files.
    ///
    /// # Errors
    ///
    /// * [`CacheBackendErrorKind::FromIoError`] - If the cache directory could not be read.
    ///
    /// [`CacheBackendErrorKind::FromIoError`]: crate::error::CacheBackendErrorKind::FromIoError
    pub fn remove_not_in_list(&self, tpe: FileType, list: &Vec<(Id, u32)>) -> RusticResult<()> {
        let mut list_cache = self.list_with_size(tpe)?;
        // remove present files from the cache list
        for (id, size) in list {
            if let Some(cached_size) = list_cache.remove(id) {
                if &cached_size != size {
                    // remove cache files with non-matching size
                    self.remove(tpe, id)?;
                }
            }
        }
        // remove all remaining (i.e. not present in repo) cache files
        for id in list_cache.keys() {
            self.remove(tpe, id)?;
        }
        Ok(())
    }

    /// Reads full data of the given file.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `id` - The id of the file.
    ///
    /// # Errors
    ///
    /// * [`CacheBackendErrorKind::FromIoError`] - If the file could not be read.
    ///
    /// [`CacheBackendErrorKind::FromIoError`]: crate::error::CacheBackendErrorKind::FromIoError
    pub fn read_full(&self, tpe: FileType, id: &Id) -> RusticResult<Bytes> {
        trace!("cache reading tpe: {:?}, id: {}", &tpe, &id);
        let data = fs::read(self.path(tpe, id)).map_err(CacheBackendErrorKind::FromIoError)?;
        trace!("cache hit!");
        Ok(data.into())
    }

    /// Reads partial data of the given file.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `id` - The id of the file.
    /// * `offset` - The offset to read from.
    /// * `length` - The length to read.
    ///
    /// # Errors
    ///
    /// * [`CacheBackendErrorKind::FromIoError`] - If the file could not be read.
    ///
    /// [`CacheBackendErrorKind::FromIoError`]: crate::error::CacheBackendErrorKind::FromIoError
    pub fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        offset: u32,
        length: u32,
    ) -> RusticResult<Bytes> {
        trace!(
            "cache reading tpe: {:?}, id: {}, offset: {}",
            &tpe,
            &id,
            &offset
        );
        let mut file =
            File::open(self.path(tpe, id)).map_err(CacheBackendErrorKind::FromIoError)?;
        _ = file
            .seek(SeekFrom::Start(u64::from(offset)))
            .map_err(CacheBackendErrorKind::FromIoError)?;
        let mut vec = vec![0; length as usize];
        file.read_exact(&mut vec)
            .map_err(CacheBackendErrorKind::FromIoError)?;
        trace!("cache hit!");
        Ok(vec.into())
    }

    /// Writes the given data to the given file.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `id` - The id of the file.
    /// * `buf` - The data to write.
    ///
    /// # Errors
    ///
    /// * [`CacheBackendErrorKind::FromIoError`] - If the file could not be written.
    ///
    /// [`CacheBackendErrorKind::FromIoError`]: crate::error::CacheBackendErrorKind::FromIoError
    pub fn write_bytes(&self, tpe: FileType, id: &Id, buf: Bytes) -> RusticResult<()> {
        trace!("cache writing tpe: {:?}, id: {}", &tpe, &id);
        fs::create_dir_all(self.dir(tpe, id)).map_err(CacheBackendErrorKind::FromIoError)?;
        let filename = self.path(tpe, id);
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(filename)
            .map_err(CacheBackendErrorKind::FromIoError)?;
        file.write_all(&buf)
            .map_err(CacheBackendErrorKind::FromIoError)?;
        Ok(())
    }

    /// Removes the given file.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `id` - The id of the file.
    ///
    /// # Errors
    ///
    /// * [`CacheBackendErrorKind::FromIoError`] - If the file could not be removed.
    ///
    /// [`CacheBackendErrorKind::FromIoError`]: crate::error::CacheBackendErrorKind::FromIoError
    pub fn remove(&self, tpe: FileType, id: &Id) -> RusticResult<()> {
        trace!("cache writing tpe: {:?}, id: {}", &tpe, &id);
        let filename = self.path(tpe, id);
        fs::remove_file(filename).map_err(CacheBackendErrorKind::FromIoError)?;
        Ok(())
    }
}
