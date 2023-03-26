use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use bytes::Bytes;
use dirs::cache_dir;
use log::*;
use walkdir::WalkDir;

use super::{FileType, Id, ReadBackend, WriteBackend};

#[derive(Clone)]
pub struct CachedBackend<BE: WriteBackend> {
    be: BE,
    cache: Option<Cache>,
}

impl<BE: WriteBackend> CachedBackend<BE> {
    pub fn new(be: BE, cache: Option<Cache>) -> Self {
        Self { be, cache }
    }
}

impl<BE: WriteBackend> ReadBackend for CachedBackend<BE> {
    fn location(&self) -> String {
        self.be.location()
    }

    fn set_option(&mut self, option: &str, value: &str) -> Result<()> {
        self.be.set_option(option, value)
    }

    fn list_with_size(&self, tpe: FileType) -> Result<Vec<(Id, u32)>> {
        let list = self.be.list_with_size(tpe)?;

        if let Some(cache) = &self.cache {
            if tpe.is_cacheable() {
                cache.remove_not_in_list(tpe, &list)?;
            }
        }

        Ok(list)
    }

    fn read_full(&self, tpe: FileType, id: &Id) -> Result<Bytes> {
        match (&self.cache, tpe.is_cacheable()) {
            (None, _) | (Some(_), false) => self.be.read_full(tpe, id),
            (Some(cache), true) => match cache.read_full(tpe, id) {
                Ok(res) => Ok(res),
                _ => {
                    let res = self.be.read_full(tpe, id);
                    if let Ok(data) = &res {
                        let _ = cache.write_bytes(tpe, id, data.clone());
                    }
                    res
                }
            },
        }
    }

    fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        cacheable: bool,
        offset: u32,
        length: u32,
    ) -> Result<Bytes> {
        match (&self.cache, cacheable || tpe.is_cacheable()) {
            (None, _) | (Some(_), false) => {
                self.be.read_partial(tpe, id, cacheable, offset, length)
            }
            (Some(cache), true) => match cache.read_partial(tpe, id, offset, length) {
                Ok(res) => Ok(res),
                _ => match self.be.read_full(tpe, id) {
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
                },
            },
        }
    }
}

impl<BE: WriteBackend> WriteBackend for CachedBackend<BE> {
    fn create(&self) -> Result<()> {
        self.be.create()
    }

    fn write_bytes(&self, tpe: FileType, id: &Id, cacheable: bool, buf: Bytes) -> Result<()> {
        if let Some(cache) = &self.cache {
            if cacheable || tpe.is_cacheable() {
                let _ = cache.write_bytes(tpe, id, buf.clone());
            }
        }
        self.be.write_bytes(tpe, id, cacheable, buf)
    }

    fn remove(&self, tpe: FileType, id: &Id, cacheable: bool) -> Result<()> {
        if let Some(cache) = &self.cache {
            if cacheable || tpe.is_cacheable() {
                let _ = cache.remove(tpe, id);
            }
        }
        self.be.remove(tpe, id, cacheable)
    }
}

#[derive(Clone)]
pub struct Cache {
    path: PathBuf,
}

impl Cache {
    pub fn new(id: Id, path: Option<PathBuf>) -> Result<Self> {
        let mut path = path.unwrap_or({
            let mut dir = cache_dir().ok_or_else(|| anyhow!("no cache dir"))?;
            dir.push("rustic");
            dir
        });
        fs::create_dir_all(&path)?;
        cachedir::ensure_tag(&path)?;
        path.push(id.to_hex());
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    pub fn location(&self) -> &str {
        self.path.to_str().unwrap()
    }

    fn dir(&self, tpe: FileType, id: &Id) -> PathBuf {
        let hex_id = id.to_hex();
        self.path.join(tpe.name()).join(&hex_id[0..2])
    }

    fn path(&self, tpe: FileType, id: &Id) -> PathBuf {
        let hex_id = id.to_hex();
        self.path.join(tpe.name()).join(&hex_id[0..2]).join(hex_id)
    }

    pub fn list_with_size(&self, tpe: FileType) -> Result<HashMap<Id, u32>> {
        let path = self.path.join(tpe.name());

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

    pub fn remove_not_in_list(&self, tpe: FileType, list: &Vec<(Id, u32)>) -> Result<()> {
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

    pub fn read_full(&self, tpe: FileType, id: &Id) -> Result<Bytes> {
        trace!("cache reading tpe: {:?}, id: {}", &tpe, &id);
        let data = fs::read(self.path(tpe, id))?;
        trace!("cache hit!");
        Ok(data.into())
    }

    fn read_partial(&self, tpe: FileType, id: &Id, offset: u32, length: u32) -> Result<Bytes> {
        trace!(
            "cache reading tpe: {:?}, id: {}, offset: {}",
            &tpe,
            &id,
            &offset
        );
        let mut file = File::open(self.path(tpe, id))?;
        file.seek(SeekFrom::Start(u64::from(offset)))?;
        let mut vec = vec![0; length as usize];
        file.read_exact(&mut vec)?;
        trace!("cache hit!");
        Ok(vec.into())
    }

    fn write_bytes(&self, tpe: FileType, id: &Id, buf: Bytes) -> Result<()> {
        trace!("cache writing tpe: {:?}, id: {}", &tpe, &id);
        fs::create_dir_all(self.dir(tpe, id))?;
        let filename = self.path(tpe, id);
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(filename)?;
        file.write_all(&buf)?;
        Ok(())
    }

    fn remove(&self, tpe: FileType, id: &Id) -> Result<()> {
        trace!("cache writing tpe: {:?}, id: {}", &tpe, &id);
        let filename = self.path(tpe, id);
        fs::remove_file(filename)?;
        Ok(())
    }
}
