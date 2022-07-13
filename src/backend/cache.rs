use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{copy, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use dirs::cache_dir;
use vlog::*;
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

#[async_trait]
impl<BE: WriteBackend> ReadBackend for CachedBackend<BE> {
    fn location(&self) -> &str {
        self.be.location()
    }

    async fn list_with_size(&self, tpe: FileType) -> Result<Vec<(Id, u32)>> {
        let list = self.be.list_with_size(tpe).await?;

        if let Some(cache) = &self.cache {
            if tpe.is_cacheable() {
                cache.remove_not_in_list(tpe, &list).await?;
            }
        }

        Ok(list)
    }

    async fn read_full(&self, tpe: FileType, id: &Id) -> Result<Vec<u8>> {
        match (&self.cache, tpe.is_cacheable()) {
            (None, _) | (Some(_), false) => self.be.read_full(tpe, id).await,
            (Some(cache), true) => match cache.read_full(tpe, id).await {
                Ok(res) => Ok(res),
                _ => {
                    let res = self.be.read_full(tpe, id).await;
                    if let Ok(data) = &res {
                        let _ = cache.write_bytes(tpe, id, data.clone()).await;
                    }
                    res
                }
            },
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
        match (&self.cache, cacheable || tpe.is_cacheable()) {
            (None, _) | (Some(_), false) => {
                self.be
                    .read_partial(tpe, id, cacheable, offset, length)
                    .await
            }
            (Some(cache), true) => match cache.read_partial(tpe, id, offset, length).await {
                Ok(res) => Ok(res),
                _ => match self.be.read_full(tpe, id).await {
                    // read full file, save to cache and return partial content from cache
                    // TODO: - Do not read to memory, but use a (Async)Reader
                    //       - Don't read from cache, but use the right part of the read content
                    Ok(data) => {
                        if cache.write_bytes(tpe, id, data.clone()).await.is_ok() {
                            cache.read_partial(tpe, id, offset, length).await
                        } else {
                            self.be.read_partial(tpe, id, false, offset, length).await
                        }
                    }
                    error => error,
                },
            },
        }
    }
}

#[async_trait]
impl<BE: WriteBackend> WriteBackend for CachedBackend<BE> {
    async fn create(&self) -> Result<()> {
        self.be.create().await
    }

    async fn write_file(&self, tpe: FileType, id: &Id, cacheable: bool, mut f: File) -> Result<()> {
        if let Some(cache) = &self.cache {
            if cacheable || tpe.is_cacheable() {
                let f_cache = f.try_clone()?;
                let _ = cache.write_file(tpe, id, cacheable, f_cache).await;
                f.seek(SeekFrom::Start(0))?;
            }
        }
        self.be.write_file(tpe, id, cacheable, f).await
    }

    async fn write_bytes(&self, tpe: FileType, id: &Id, buf: Vec<u8>) -> Result<()> {
        if let Some(cache) = &self.cache {
            if tpe.is_cacheable() {
                let _ = cache.write_bytes(tpe, id, buf.clone()).await;
            }
        }
        self.be.write_bytes(tpe, id, buf).await
    }

    async fn remove(&self, tpe: FileType, id: &Id) -> Result<()> {
        if let Some(cache) = &self.cache {
            let _ = cache.remove(tpe, id).await;
        }
        self.be.remove(tpe, id).await
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
        self.path.join(tpe.name()).join(&hex_id[0..2]).join(&hex_id)
    }

    pub async fn list_with_size(&self, tpe: FileType) -> Result<HashMap<Id, u32>> {
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
                        .into_iter()
                        .all(|c| ('0'..='9').contains(&c) || ('a'..='f').contains(&c))
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

    // TODO: this function is yet only called from list_with_size. This cleans up
    // index and snapshot files.
    // It should also be called when reading the index to clean up pack files.

    pub async fn remove_not_in_list(&self, tpe: FileType, list: &Vec<(Id, u32)>) -> Result<()> {
        let mut list_cache = self.list_with_size(tpe).await?;
        // remove present files from the cache list
        for (id, size) in list {
            if let Some(cached_size) = list_cache.remove(id) {
                if &cached_size != size {
                    // remove cache files with non-matching size
                    self.remove(tpe, id).await?;
                }
            }
        }
        // remove all remaining (i.e. not present in repo) cache files
        for id in list_cache.keys() {
            self.remove(tpe, id).await?;
        }
        Ok(())
    }

    pub async fn read_full(&self, tpe: FileType, id: &Id) -> Result<Vec<u8>> {
        v3!("cache reading tpe: {:?}, id: {}", &tpe, &id);
        let data = fs::read(self.path(tpe, id))?;
        v3!("cache hit!");
        Ok(data)
    }

    async fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>> {
        v3!(
            "cache reading tpe: {:?}, id: {}, offset: {}",
            &tpe,
            &id,
            &offset
        );
        let mut file = File::open(self.path(tpe, id))?;
        file.seek(SeekFrom::Start(offset as u64))?;
        let mut vec = vec![0; length as usize];
        file.read_exact(&mut vec)?;
        v3!("cache hit!");
        Ok(vec)
    }

    async fn write_file(
        &self,
        tpe: FileType,
        id: &Id,
        _cacheable: bool,
        mut f: File,
    ) -> Result<()> {
        v3!("cache writing tpe: {:?}, id: {}", &tpe, &id);
        fs::create_dir_all(self.dir(tpe, id))?;
        let filename = self.path(tpe, id);
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&filename)?;
        copy(&mut f, &mut file)?;
        Ok(())
    }

    async fn write_bytes(&self, tpe: FileType, id: &Id, buf: Vec<u8>) -> Result<()> {
        v3!("cache writing tpe: {:?}, id: {}", &tpe, &id);
        fs::create_dir_all(self.dir(tpe, id))?;
        let filename = self.path(tpe, id);
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&filename)?;
        file.write_all(&buf)?;
        Ok(())
    }

    async fn remove(&self, tpe: FileType, id: &Id) -> Result<()> {
        v3!("cache writing tpe: {:?}, id: {}", &tpe, &id);
        let filename = self.path(tpe, id);
        fs::remove_file(filename)?;
        Ok(())
    }
}
