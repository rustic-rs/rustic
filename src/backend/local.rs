use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
#[cfg(not(windows))]
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

use aho_corasick::AhoCorasick;
#[cfg(not(any(windows, target_os = "openbsd")))]
use anyhow::Context;
use anyhow::{anyhow, bail, Result};
use bytes::Bytes;
use filetime::{set_symlink_file_times, FileTime};
use log::*;
#[cfg(not(windows))]
use nix::sys::stat::{mknod, Mode, SFlag};
#[cfg(not(windows))]
use nix::unistd::{fchownat, FchownatFlags, Gid, Group, Uid, User};
use walkdir::WalkDir;

use crate::repository::parse_command;

#[cfg(not(windows))]
use super::mapper::map_mode_from_go;
#[cfg(not(windows))]
use super::node::NodeType;
use super::node::{ExtendedAttribute, Metadata, Node};
use super::{FileType, Id, ReadBackend, WriteBackend, ALL_FILE_TYPES};

#[derive(Clone)]
pub struct LocalBackend {
    path: PathBuf,
    post_create_command: Option<String>,
    post_delete_command: Option<String>,
}

impl LocalBackend {
    pub fn new(path: &str) -> Result<Self> {
        let path = path.into();
        fs::create_dir_all(&path)?;
        Ok(Self {
            path,
            post_create_command: None,
            post_delete_command: None,
        })
    }

    fn path(&self, tpe: FileType, id: &Id) -> PathBuf {
        let hex_id = id.to_hex();
        match tpe {
            FileType::Config => self.path.join("config"),
            FileType::Pack => self.path.join("data").join(&hex_id[0..2]).join(hex_id),
            _ => self.path.join(tpe.name()).join(hex_id),
        }
    }

    fn call_command(&self, tpe: FileType, id: &Id, filename: &Path, command: &str) -> Result<()> {
        let id = id.to_hex();
        let patterns = &["%file", "%type", "%id"];
        let ac = AhoCorasick::new(patterns)?;
        let replace_with = &[filename.to_str().unwrap(), tpe.name(), id.as_str()];
        let actual_command = ac.replace_all(command, replace_with);
        debug!("calling {actual_command}...");
        let commands = parse_command::<()>(&actual_command)?.1;
        let status = Command::new(commands[0]).args(&commands[1..]).status()?;
        if !status.success() {
            bail!(
                "command was not successful for filename {}, type {}, id {}. {status}",
                replace_with[0],
                replace_with[1],
                replace_with[2]
            );
        }
        Ok(())
    }
}

impl ReadBackend for LocalBackend {
    fn location(&self) -> String {
        let mut location = "local:".to_string();
        location.push_str(&self.path.to_string_lossy());
        location
    }

    fn set_option(&mut self, option: &str, value: &str) -> Result<()> {
        match option {
            "post-create-command" => {
                self.post_create_command = Some(value.to_string());
            }
            "post-delete-command" => {
                self.post_delete_command = Some(value.to_string());
            }
            opt => {
                warn!("Option {opt} is not supported! Ignoring it.");
            }
        }
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
            .open(&filename)?;
        file.set_len(buf.len().try_into()?)?;
        file.write_all(&buf)?;
        file.sync_all()?;
        if let Some(command) = &self.post_create_command {
            if let Err(err) = self.call_command(tpe, id, &filename, command) {
                warn!("post-create: {err}");
            }
        }
        Ok(())
    }

    fn remove(&self, tpe: FileType, id: &Id, _cacheable: bool) -> Result<()> {
        trace!("removing tpe: {:?}, id: {}", &tpe, &id);
        let filename = self.path(tpe, id);
        fs::remove_file(&filename)?;
        if let Some(command) = &self.post_delete_command {
            if let Err(err) = self.call_command(tpe, id, &filename, command) {
                warn!("post-delete: {err}");
            }
        }
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
        if let Some(mtime) = meta.mtime {
            let atime = meta.atime.unwrap_or(mtime);
            set_symlink_file_times(
                filename,
                FileTime::from_system_time(atime.into()),
                FileTime::from_system_time(mtime.into()),
            )?;
        }

        Ok(())
    }

    #[cfg(windows)]
    // TODO
    pub fn set_user_group(&self, _item: impl AsRef<Path>, _meta: &Metadata) -> Result<()> {
        Ok(())
    }

    #[cfg(not(windows))]
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
        fchownat(None, &filename, uid, gid, FchownatFlags::NoFollowSymlink)?;
        Ok(())
    }

    #[cfg(windows)]
    // TODO
    pub fn set_uid_gid(&self, _item: impl AsRef<Path>, _meta: &Metadata) -> Result<()> {
        Ok(())
    }

    #[cfg(not(windows))]
    pub fn set_uid_gid(&self, item: impl AsRef<Path>, meta: &Metadata) -> Result<()> {
        let filename = self.path(item);

        let uid = meta.uid.map(Uid::from_raw);
        let gid = meta.gid.map(Gid::from_raw);

        fchownat(None, &filename, uid, gid, FchownatFlags::NoFollowSymlink)?;
        Ok(())
    }

    #[cfg(windows)]
    // TODO
    pub fn set_permission(&self, _item: impl AsRef<Path>, _node: &Node) -> Result<()> {
        Ok(())
    }

    #[cfg(not(windows))]
    pub fn set_permission(&self, item: impl AsRef<Path>, node: &Node) -> Result<()> {
        if node.node_type.is_symlink() {
            return Ok(());
        }

        let filename = self.path(item);

        if let Some(mode) = node.meta.mode {
            let mode = map_mode_from_go(mode);
            std::fs::set_permissions(filename, fs::Permissions::from_mode(mode))?;
        }
        Ok(())
    }

    #[cfg(any(windows, target_os = "openbsd"))]
    pub fn set_extended_attributes(
        &self,
        _item: impl AsRef<Path>,
        _extended_attributes: &[ExtendedAttribute],
    ) -> Result<()> {
        Ok(())
    }

    #[cfg(not(any(windows, target_os = "openbsd")))]
    pub fn set_extended_attributes(
        &self,
        item: impl AsRef<Path>,
        extended_attributes: &[ExtendedAttribute],
    ) -> Result<()> {
        let filename = self.path(item);
        let mut done = vec![false; extended_attributes.len()];

        for curr_name in
            xattr::list(&filename).with_context(|| format!("listing xattrs on {filename:?}"))?
        {
            match extended_attributes.iter().enumerate().find(
                |(_, ExtendedAttribute { name, .. })| name == curr_name.to_string_lossy().as_ref(),
            ) {
                Some((index, ExtendedAttribute { name, value })) => {
                    let curr_value = xattr::get(&filename, name)?.unwrap();
                    if value != &curr_value {
                        xattr::set(&filename, name, value)
                            .with_context(|| format!("setting xattr {name} on {filename:?}"))?;
                    }
                    done[index] = true;
                }
                None => {
                    if let Err(err) = xattr::remove(&filename, &curr_name) {
                        warn!("error removing xattr {curr_name:?} on {filename:?}: {err}");
                    }
                }
            }
        }

        for (index, ExtendedAttribute { name, value }) in extended_attributes.iter().enumerate() {
            if !done[index] {
                xattr::set(&filename, name, value)
                    .with_context(|| format!("setting xattr {name} on {filename:?}"))?;
            }
        }

        Ok(())
    }

    // set_length sets the length of the given file. If it doesn't exist, create a new (empty) one with given length
    pub fn set_length(&self, item: impl AsRef<Path>, size: u64) -> Result<()> {
        let filename = self.path(item);
        let dir = filename
            .parent()
            .ok_or_else(|| anyhow!("file {filename:?} should have a parent"))?;
        fs::create_dir_all(dir)?;

        OpenOptions::new()
            .create(true)
            .write(true)
            .open(filename)?
            .set_len(size)?;
        Ok(())
    }

    #[cfg(windows)]
    // TODO
    pub fn create_special(&self, _item: impl AsRef<Path>, _node: &Node) -> Result<()> {
        Ok(())
    }

    #[cfg(not(windows))]
    pub fn create_special(&self, item: impl AsRef<Path>, node: &Node) -> Result<()> {
        let filename = self.path(item);

        match &node.node_type {
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
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(filename)?;
        file.seek(SeekFrom::Start(offset))?;
        file.write_all(data)?;
        Ok(())
    }
}
