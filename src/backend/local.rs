use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{symlink, FileExt, PermissionsExt};
use std::path::{Path, PathBuf};

use anyhow::Result;
use bytes::Bytes;
use filetime::{set_file_atime, set_file_mtime, FileTime};
use log::*;
use nix::sys::stat::{mknod, Mode, SFlag};
use nix::unistd::chown;
use nix::unistd::{Gid, Group, Uid, User};
use walkdir::WalkDir;

use super::node::{Metadata, Node, NodeType};
use super::{map_mode_from_go, FileType, Id, ReadBackend, WriteBackend, ALL_FILE_TYPES};

#[derive(Clone)]
pub struct LocalBackend {
    path: PathBuf,
}

impl LocalBackend {
    pub fn new(path: &str) -> Result<Self> {
        let path = path.into();
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    fn path(&self, tpe: FileType, id: &Id) -> PathBuf {
        let hex_id = id.to_hex();
        match tpe {
            FileType::Config => self.path.join("config"),
            FileType::Pack => self.path.join("data").join(&hex_id[0..2]).join(hex_id),
            _ => self.path.join(tpe.name()).join(hex_id),
        }
    }
}

impl ReadBackend for LocalBackend {
    fn location(&self) -> &str {
        self.path.to_str().unwrap()
    }

    fn set_option(&mut self, _option: &str, _value: &str) -> Result<()> {
        Ok(())
    }

    fn list(&self, tpe: FileType) -> Result<Vec<Id>> {
        trace!("listing tpe: {tpe:?}");
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

    fn list_with_size(&self, tpe: FileType) -> Result<Vec<(Id, u32)>> {
        trace!("listing tpe: {tpe:?}");
        let path = self.path.join(tpe.name());

        if tpe == FileType::Config {
            return Ok(match path.exists() {
                true => vec![(Id::default(), path.metadata()?.len().try_into()?)],
                false => Vec::new(),
            });
        }

        let walker = WalkDir::new(path)
            .into_iter()
            .filter_map(walkdir::Result::ok)
            .filter(|e| e.file_type().is_file())
            .map(|e| -> Result<_> {
                Ok((
                    Id::from_hex(&e.file_name().to_string_lossy())?,
                    e.metadata()?.len().try_into()?,
                ))
            })
            .filter_map(Result::ok);

        Ok(walker.collect())
    }

    fn read_full(&self, tpe: FileType, id: &Id) -> Result<Bytes> {
        trace!("reading tpe: {tpe:?}, id: {id}");
        Ok(fs::read(self.path(tpe, id))?.into())
    }

    fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        _cacheable: bool,
        offset: u32,
        length: u32,
    ) -> Result<Bytes> {
        trace!("reading tpe: {tpe:?}, id: {id}, offset: {offset}, length: {length}");
        let mut file = File::open(self.path(tpe, id))?;
        file.seek(SeekFrom::Start(offset.try_into()?))?;
        let mut vec = vec![0; length.try_into()?];
        file.read_exact(&mut vec)?;
        Ok(vec.into())
    }
}

impl WriteBackend for LocalBackend {
    fn create(&self) -> Result<()> {
        trace!("creating repo at {:?}", self.path);

        for tpe in ALL_FILE_TYPES {
            fs::create_dir_all(self.path.join(tpe.name()))?;
        }
        for i in 0u8..=255 {
            fs::create_dir_all(self.path.join("data").join(hex::encode([i])))?;
        }
        Ok(())
    }

    fn write_bytes(&self, tpe: FileType, id: &Id, _cacheable: bool, buf: Bytes) -> Result<()> {
        trace!("writing tpe: {:?}, id: {}", &tpe, &id);
        let filename = self.path(tpe, id);
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(filename)?;
        file.set_len(buf.len().try_into()?)?;
        file.write_all(&buf)?;
        file.sync_all()?;
        Ok(())
    }

    fn remove(&self, tpe: FileType, id: &Id, _cacheable: bool) -> Result<()> {
        trace!("removing tpe: {:?}, id: {}", &tpe, &id);
        let filename = self.path(tpe, id);
        fs::remove_file(filename)?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct LocalDestination {
    path: PathBuf,
    is_file: bool,
}

impl LocalDestination {
    pub fn new(path: &str, create: bool, expect_file: bool) -> Result<Self> {
        let is_dir = path.ends_with('/');
        let path: PathBuf = path.into();
        let is_file = path.is_file() || (!path.is_dir() && !is_dir && expect_file);

        if create {
            if is_file {
                if let Some(path) = path.parent() {
                    fs::create_dir_all(path)?;
                }
            } else {
                fs::create_dir_all(&path)?;
            }
        }

        Ok(Self { path, is_file })
    }

    fn path(&self, item: impl AsRef<Path>) -> PathBuf {
        if self.is_file {
            self.path.clone()
        } else {
            self.path.join(item)
        }
    }

    pub fn remove_dir(&self, dirname: impl AsRef<Path>) -> Result<()> {
        Ok(fs::remove_dir_all(dirname)?)
    }

    pub fn remove_file(&self, filename: impl AsRef<Path>) -> Result<()> {
        Ok(fs::remove_file(filename)?)
    }

    pub fn create_dir(&self, item: impl AsRef<Path>) -> Result<()> {
        let dirname = self.path.join(item);
        fs::create_dir_all(dirname)?;
        Ok(())
    }

    pub fn set_times(&self, item: impl AsRef<Path>, meta: &Metadata) -> Result<()> {
        let filename = self.path(item);
        if let Some(mtime) = meta.mtime.map(|t| FileTime::from_system_time(t.into())) {
            set_file_mtime(&filename, mtime)?;
        }
        if let Some(atime) = meta.atime.map(|t| FileTime::from_system_time(t.into())) {
            set_file_atime(filename, atime)?;
        }
        Ok(())
    }

    pub fn set_user_group(&self, item: impl AsRef<Path>, meta: &Metadata) -> Result<()> {
        let filename = self.path(item);

        let user = meta
            .user
            .as_ref()
            .and_then(|name| User::from_name(name).unwrap());

        // use uid from user if valid, else from saved uid (if saved)
        let uid = user.map(|u| u.uid).or_else(|| meta.uid.map(Uid::from_raw));

        let group = meta
            .group
            .as_ref()
            .and_then(|name| Group::from_name(name).unwrap());
        // use gid from group if valid, else from saved gid (if saved)
        let gid = group.map(|g| g.gid).or_else(|| meta.gid.map(Gid::from_raw));

        chown(&filename, uid, gid)?;
        Ok(())
    }

    pub fn set_uid_gid(&self, item: impl AsRef<Path>, meta: &Metadata) -> Result<()> {
        let filename = self.path(item);

        let uid = meta.uid.map(Uid::from_raw);
        let gid = meta.gid.map(Gid::from_raw);

        chown(&filename, uid, gid)?;
        Ok(())
    }

    pub fn set_permission(&self, item: impl AsRef<Path>, meta: &Metadata) -> Result<()> {
        let filename = self.path(item);

        if let Some(mode) = meta.mode() {
            let mode = map_mode_from_go(*mode);
            std::fs::set_permissions(filename, fs::Permissions::from_mode(mode))?;
        }
        Ok(())
    }

    // set_length sets the length of the given file. If it doesn't exist, create a new (empty) one with given length
    pub fn set_length(&self, item: impl AsRef<Path>, size: u64) -> Result<()> {
        let filename = self.path(item);
        OpenOptions::new()
            .create(true)
            .write(true)
            .open(filename)?
            .set_len(size)?;
        Ok(())
    }

    pub fn create_special(&self, item: impl AsRef<Path>, node: &Node) -> Result<()> {
        let filename = self.path(item);

        match node.node_type() {
            NodeType::Symlink { linktarget } => {
                symlink(linktarget, filename)?;
            }
            NodeType::Dev { device } => {
                #[cfg(not(any(
                    target_os = "macos",
                    target_os = "openbsd",
                    target_os = "freebsd"
                )))]
                let device = *device;
                #[cfg(any(target_os = "macos", target_os = "openbsd"))]
                let device = i32::try_from(*device)?;
                #[cfg(target_os = "freebsd")]
                let device = u32::try_from(*device)?;
                mknod(&filename, SFlag::S_IFBLK, Mode::empty(), device)?;
            }
            NodeType::Chardev { device } => {
                #[cfg(not(any(
                    target_os = "macos",
                    target_os = "openbsd",
                    target_os = "freebsd"
                )))]
                let device = *device;
                #[cfg(any(target_os = "macos", target_os = "openbsd"))]
                let device = i32::try_from(*device)?;
                #[cfg(target_os = "freebsd")]
                let device = u32::try_from(*device)?;
                mknod(&filename, SFlag::S_IFCHR, Mode::empty(), device)?;
            }
            NodeType::Fifo => {
                mknod(&filename, SFlag::S_IFIFO, Mode::empty(), 0)?;
            }
            NodeType::Socket => {
                mknod(&filename, SFlag::S_IFSOCK, Mode::empty(), 0)?;
            }
            _ => {}
        }
        Ok(())
    }

    pub fn read_at(&self, item: impl AsRef<Path>, offset: u64, length: u64) -> Result<Bytes> {
        let filename = self.path(item);
        let mut file = File::open(filename)?;
        file.seek(SeekFrom::Start(offset))?;
        let mut vec = vec![0; length.try_into()?];
        file.read_exact(&mut vec)?;
        Ok(vec.into())
    }

    pub fn get_matching_file(&self, item: impl AsRef<Path>, size: u64) -> Option<File> {
        let filename = self.path(item);
        match fs::symlink_metadata(&filename) {
            Ok(meta) => {
                if meta.is_file() && meta.len() == size {
                    File::open(&filename).ok()
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }

    pub fn write_at(&self, item: impl AsRef<Path>, offset: u64, data: &[u8]) -> Result<()> {
        let filename = self.path(item);
        let file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(filename)?;
        file.write_all_at(data, offset)?;
        Ok(())
    }
}
