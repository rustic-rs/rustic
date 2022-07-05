use std::fs::{self, File};
use std::io::{copy, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{symlink, FileExt, PermissionsExt};
use std::path::{Path, PathBuf};

use anyhow::Result;
use async_trait::async_trait;
use nix::sys::stat::{mknod, Mode, SFlag};
use vlog::*;
use walkdir::WalkDir;

use super::node::{Metadata, Node, NodeType};
use super::{map_mode_from_go, FileType, Id, ReadBackend, WriteBackend, ALL_FILE_TYPES};

#[derive(Clone)]
pub struct LocalBackend {
    path: PathBuf,
}

impl LocalBackend {
    pub fn new(path: &str) -> Self {
        Self { path: path.into() }
    }

    fn path(&self, tpe: FileType, id: &Id) -> PathBuf {
        let hex_id = id.to_hex();
        match tpe {
            FileType::Config => self.path.join("config"),
            FileType::Pack => self.path.join("data").join(&hex_id[0..2]).join(&hex_id),
            _ => self.path.join(tpe.name()).join(&hex_id),
        }
    }
}

#[async_trait]
impl ReadBackend for LocalBackend {
    fn location(&self) -> &str {
        self.path.to_str().unwrap()
    }

    async fn list(&self, tpe: FileType) -> Result<Vec<Id>> {
        if tpe == FileType::Config {
            return Ok(match self.path.join("config").exists() {
                true => vec![Id::default()],
                false => Vec::new(),
            });
        }

        let walker = WalkDir::new(self.path.join(tpe.name()))
            .into_iter()
            .filter_map(walkdir::Result::ok)
            .filter(|e| e.file_type().is_file())
            .map(|e| Id::from_hex(&e.file_name().to_string_lossy()))
            .filter_map(Result::ok);
        Ok(walker.collect())
    }

    async fn list_with_size(&self, tpe: FileType) -> Result<Vec<(Id, u32)>> {
        let path = self.path.join(tpe.name());

        if tpe == FileType::Config {
            return Ok(match path.exists() {
                true => vec![(
                    Id::default(),
                    path.metadata().unwrap().len().try_into().unwrap(),
                )],
                false => Vec::new(),
            });
        }

        let walker = WalkDir::new(path)
            .into_iter()
            .filter_map(walkdir::Result::ok)
            .filter(|e| {
                // only use files with length of 64 which are valid hex
                // TODO: maybe add an option which warns if other files exist?
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
                    e.metadata().unwrap().len().try_into().unwrap(),
                )
            });

        Ok(walker.collect())
    }

    async fn read_full(&self, tpe: FileType, id: &Id) -> Result<Vec<u8>> {
        Ok(fs::read(self.path(tpe, id))?)
    }

    async fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        _cacheable: bool,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>> {
        let mut file = File::open(self.path(tpe, id))?;
        file.seek(SeekFrom::Start(offset.try_into().unwrap()))?;
        let mut vec = vec![0; length.try_into().unwrap()];
        file.read_exact(&mut vec)?;
        Ok(vec)
    }
}

#[async_trait]
impl WriteBackend for LocalBackend {
    async fn create(&self) -> Result<()> {
        for tpe in ALL_FILE_TYPES {
            fs::create_dir_all(self.path.join(tpe.name()))?;
        }
        for i in 0u8..=255 {
            fs::create_dir_all(self.path.join("data").join(hex::encode([i])))?;
        }
        Ok(())
    }

    async fn write_file(
        &self,
        tpe: FileType,
        id: &Id,
        _cacheable: bool,
        mut f: File,
    ) -> Result<()> {
        v3!("writing tpe: {:?}, id: {}", &tpe, &id);
        let filename = self.path(tpe, id);
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&filename)?;
        copy(&mut f, &mut file)?;
        file.sync_all()?;
        Ok(())
    }

    async fn write_bytes(&self, tpe: FileType, id: &Id, buf: Vec<u8>) -> Result<()> {
        v3!("writing tpe: {:?}, id: {}", &tpe, &id);
        let filename = self.path(tpe, id);
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&filename)?;
        file.write_all(&buf)?;
        file.sync_all()?;
        Ok(())
    }

    async fn remove(&self, tpe: FileType, id: &Id) -> Result<()> {
        v3!("writing tpe: {:?}, id: {}", &tpe, &id);
        let filename = self.path(tpe, id);
        fs::remove_file(filename)?;
        Ok(())
    }
}

impl LocalBackend {
    /*
        pub fn walker(&self) -> impl Iterator<Item = PathBuf> {
            let path = self.path.clone();
            WalkDir::new(path.clone())
                .min_depth(1)
                .into_iter()
                .filter_map(walkdir::Result::ok)
                .map(move |e| e.path().strip_prefix(path.clone()).unwrap().into())
        }

        pub fn remove_dir(&self, item: impl AsRef<Path>) {
            let dirname = self.path.join(item);
            fs::remove_dir(&dirname).unwrap();
        }

        pub fn remove_file(&self, item: impl AsRef<Path>) {
            let filename = self.path.join(item);
            fs::remove_file(&filename).unwrap();
        }
    */

    pub fn create_dir(&self, item: impl AsRef<Path>) {
        let dirname = self.path.join(item);
        fs::create_dir(&dirname).unwrap();
    }

    // TODO: uid/gid and times
    pub fn set_metadata(&self, item: impl AsRef<Path>, meta: &Metadata) {
        let filename = self.path.join(item);
        let mode = map_mode_from_go(*meta.mode());
        std::fs::set_permissions(&filename, fs::Permissions::from_mode(mode))
            .unwrap_or_else(|_| panic!("error chmod {:?}", filename));
    }

    pub fn create_file(&self, item: impl AsRef<Path>, size: u64) {
        let filename = self.path.join(item);
        let f = fs::File::create(filename).unwrap();
        f.set_len(size).unwrap();
    }

    pub fn create_special(&self, item: impl AsRef<Path>, node: &Node) {
        let filename = self.path.join(item);

        match node.node_type() {
            NodeType::Symlink { linktarget } => {
                symlink(linktarget, filename).unwrap();
            }
            NodeType::Dev { device } => {
                #[cfg(not(target_os = "macos"))]
                let device = *device;
                #[cfg(target_os = "macos")]
                let device = *device as i32;
                mknod(&filename, SFlag::S_IFBLK, Mode::empty(), device).unwrap();
            }
            NodeType::Chardev { device } => {
                #[cfg(not(target_os = "macos"))]
                let device = *device;
                #[cfg(target_os = "macos")]
                let device = *device as i32;
                mknod(&filename, SFlag::S_IFCHR, Mode::empty(), device).unwrap();
            }
            NodeType::Fifo => {
                mknod(&filename, SFlag::S_IFIFO, Mode::empty(), 0).unwrap();
            }
            NodeType::Socket => {
                mknod(&filename, SFlag::S_IFSOCK, Mode::empty(), 0).unwrap();
            }
            _ => {}
        }
    }

    pub fn write_at(&self, item: impl AsRef<Path>, offset: u64, data: &[u8]) {
        let filename = self.path.join(item);
        let file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&filename)
            .unwrap();
        file.write_all_at(data, offset).unwrap();
    }
}
