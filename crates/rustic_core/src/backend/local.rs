#[cfg(not(windows))]
use std::os::unix::fs::{symlink, PermissionsExt};

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::Command,
};

use aho_corasick::AhoCorasick;
use bytes::Bytes;
use filetime::{set_symlink_file_times, FileTime};
use log::{debug, trace, warn};
#[cfg(not(windows))]
use nix::sys::stat::{mknod, Mode, SFlag};
#[cfg(not(windows))]
use nix::unistd::{fchownat, FchownatFlags, Gid, Group, Uid, User};
use walkdir::WalkDir;

#[cfg(not(windows))]
use crate::backend::ignore::mapper::map_mode_from_go;
#[cfg(not(windows))]
use crate::backend::node::NodeType;

use crate::{
    backend::{
        node::{ExtendedAttribute, Metadata, Node},
        FileType, Id, ReadBackend, WriteBackend, ALL_FILE_TYPES,
    },
    error::LocalErrorKind,
    repository::parse_command,
    RusticResult,
};

#[derive(Clone, Debug)]
pub struct LocalBackend {
    path: PathBuf,
    post_create_command: Option<String>,
    post_delete_command: Option<String>,
}

impl LocalBackend {
    pub fn new(path: &str) -> RusticResult<Self> {
        let path = path.into();
        fs::create_dir_all(&path).map_err(LocalErrorKind::DirectoryCreationFailed)?;
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
            _ => self.path.join(tpe.to_string()).join(hex_id),
        }
    }

    fn call_command(tpe: FileType, id: &Id, filename: &Path, command: &str) -> RusticResult<()> {
        let id = id.to_hex();
        let patterns = &["%file", "%type", "%id"];
        let ac = AhoCorasick::new(patterns).map_err(LocalErrorKind::FromAhoCorasick)?;
        let replace_with = &[filename.to_str().unwrap(), tpe.into(), id.as_str()];
        let actual_command = ac.replace_all(command, replace_with);
        debug!("calling {actual_command}...");
        let commands = parse_command::<()>(&actual_command)
            .map_err(LocalErrorKind::FromNomError)?
            .1;
        let status = Command::new(commands[0])
            .args(&commands[1..])
            .status()
            .map_err(LocalErrorKind::CommandExecutionFailed)?;
        if !status.success() {
            return Err(LocalErrorKind::CommandNotSuccessful {
                file_name: replace_with[0].to_owned(),
                file_type: replace_with[1].to_owned(),
                id: replace_with[2].to_owned(),
                status,
            }
            .into());
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

    fn set_option(&mut self, option: &str, value: &str) -> RusticResult<()> {
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

    fn list(&self, tpe: FileType) -> RusticResult<Vec<Id>> {
        trace!("listing tpe: {tpe:?}");
        if tpe == FileType::Config {
            return Ok(if self.path.join("config").exists() {
                vec![Id::default()]
            } else {
                Vec::new()
            });
        }

        let walker = WalkDir::new(self.path.join(tpe.to_string()))
            .into_iter()
            .filter_map(walkdir::Result::ok)
            .filter(|e| e.file_type().is_file())
            .map(|e| Id::from_hex(&e.file_name().to_string_lossy()))
            .filter_map(std::result::Result::ok);
        Ok(walker.collect())
    }

    fn list_with_size(&self, tpe: FileType) -> RusticResult<Vec<(Id, u32)>> {
        trace!("listing tpe: {tpe:?}");
        let path = self.path.join(tpe.to_string());

        if tpe == FileType::Config {
            return Ok(if path.exists() {
                vec![(
                    Id::default(),
                    path.metadata()
                        .map_err(LocalErrorKind::QueryingMetadataFailed)?
                        .len()
                        .try_into()
                        .map_err(LocalErrorKind::FromTryIntError)?,
                )]
            } else {
                Vec::new()
            });
        }

        let walker = WalkDir::new(path)
            .into_iter()
            .filter_map(walkdir::Result::ok)
            .filter(|e| e.file_type().is_file())
            .map(|e| -> RusticResult<_> {
                Ok((
                    Id::from_hex(&e.file_name().to_string_lossy())?,
                    e.metadata()
                        .map_err(LocalErrorKind::QueryingWalkDirMetadataFailed)?
                        .len()
                        .try_into()
                        .map_err(LocalErrorKind::FromTryIntError)?,
                ))
            })
            .filter_map(RusticResult::ok);

        Ok(walker.collect())
    }

    fn read_full(&self, tpe: FileType, id: &Id) -> RusticResult<Bytes> {
        trace!("reading tpe: {tpe:?}, id: {id}");
        Ok(fs::read(self.path(tpe, id))
            .map_err(LocalErrorKind::ReadingContentsOfFileFailed)?
            .into())
    }

    fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        _cacheable: bool,
        offset: u32,
        length: u32,
    ) -> RusticResult<Bytes> {
        trace!("reading tpe: {tpe:?}, id: {id}, offset: {offset}, length: {length}");
        let mut file = File::open(self.path(tpe, id)).map_err(LocalErrorKind::OpeningFileFailed)?;
        _ = file
            .seek(SeekFrom::Start(
                offset
                    .try_into()
                    .expect("offset conversion should never fail."),
            ))
            .map_err(LocalErrorKind::CouldNotSeekToPositionInFile)?;
        let mut vec = vec![0; length.try_into().map_err(LocalErrorKind::FromTryIntError)?];
        file.read_exact(&mut vec)
            .map_err(LocalErrorKind::ReadingExactLengthOfFileFailed)?;
        Ok(vec.into())
    }
}

impl WriteBackend for LocalBackend {
    fn create(&self) -> RusticResult<()> {
        trace!("creating repo at {:?}", self.path);

        for tpe in ALL_FILE_TYPES {
            fs::create_dir_all(self.path.join(tpe.to_string()))
                .map_err(LocalErrorKind::DirectoryCreationFailed)?;
        }
        for i in 0u8..=255 {
            fs::create_dir_all(self.path.join("data").join(hex::encode([i])))
                .map_err(LocalErrorKind::DirectoryCreationFailed)?;
        }
        Ok(())
    }

    fn write_bytes(
        &self,
        tpe: FileType,
        id: &Id,
        _cacheable: bool,
        buf: Bytes,
    ) -> RusticResult<()> {
        trace!("writing tpe: {:?}, id: {}", &tpe, &id);
        let filename = self.path(tpe, id);
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&filename)
            .map_err(LocalErrorKind::OpeningFileFailed)?;
        file.set_len(
            buf.len()
                .try_into()
                .map_err(LocalErrorKind::FromTryIntError)?,
        )
        .map_err(LocalErrorKind::SettingFileLengthFailed)?;
        file.write_all(&buf)
            .map_err(LocalErrorKind::CouldNotWriteToBuffer)?;
        file.sync_all()
            .map_err(LocalErrorKind::SyncingOfOsMetadataFailed)?;
        if let Some(command) = &self.post_create_command {
            if let Err(err) = Self::call_command(tpe, id, &filename, command) {
                warn!("post-create: {err}");
            }
        }
        Ok(())
    }

    fn remove(&self, tpe: FileType, id: &Id, _cacheable: bool) -> RusticResult<()> {
        trace!("removing tpe: {:?}, id: {}", &tpe, &id);
        let filename = self.path(tpe, id);
        fs::remove_file(&filename).map_err(LocalErrorKind::FileRemovalFailed)?;
        if let Some(command) = &self.post_delete_command {
            if let Err(err) = Self::call_command(tpe, id, &filename, command) {
                warn!("post-delete: {err}");
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct LocalDestination {
    path: PathBuf,
    is_file: bool,
}

impl LocalDestination {
    pub fn new(path: &str, create: bool, expect_file: bool) -> RusticResult<Self> {
        let is_dir = path.ends_with('/');
        let path: PathBuf = path.into();
        let is_file = path.is_file() || (!path.is_dir() && !is_dir && expect_file);

        if create {
            if is_file {
                if let Some(path) = path.parent() {
                    fs::create_dir_all(path).map_err(LocalErrorKind::DirectoryCreationFailed)?;
                }
            } else {
                fs::create_dir_all(&path).map_err(LocalErrorKind::DirectoryCreationFailed)?;
            }
        }

        Ok(Self { path, is_file })
    }

    pub(crate) fn path(&self, item: impl AsRef<Path>) -> PathBuf {
        if self.is_file {
            self.path.clone()
        } else {
            self.path.join(item)
        }
    }

    pub fn remove_dir(&self, dirname: impl AsRef<Path>) -> RusticResult<()> {
        Ok(fs::remove_dir_all(dirname).map_err(LocalErrorKind::DirectoryRemovalFailed)?)
    }

    pub fn remove_file(&self, filename: impl AsRef<Path>) -> RusticResult<()> {
        Ok(fs::remove_file(filename).map_err(LocalErrorKind::FileRemovalFailed)?)
    }

    pub fn create_dir(&self, item: impl AsRef<Path>) -> RusticResult<()> {
        let dirname = self.path.join(item);
        fs::create_dir_all(dirname).map_err(LocalErrorKind::DirectoryCreationFailed)?;
        Ok(())
    }

    pub fn set_times(&self, item: impl AsRef<Path>, meta: &Metadata) -> RusticResult<()> {
        let filename = self.path(item);
        if let Some(mtime) = meta.mtime {
            let atime = meta.atime.unwrap_or(mtime);
            set_symlink_file_times(
                filename,
                FileTime::from_system_time(atime.into()),
                FileTime::from_system_time(mtime.into()),
            )
            .map_err(LocalErrorKind::SettingTimeMetadataFailed)?;
        }

        Ok(())
    }

    #[cfg(windows)]
    // TODO: Windows support
    pub fn set_user_group(&self, _item: impl AsRef<Path>, _meta: &Metadata) -> RusticResult<()> {
        // https://learn.microsoft.com/en-us/windows/win32/fileio/file-security-and-access-rights
        // https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Security/struct.SECURITY_ATTRIBUTES.html
        // https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Storage/FileSystem/struct.CREATEFILE2_EXTENDED_PARAMETERS.html#structfield.lpSecurityAttributes
        Ok(())
    }

    #[cfg(not(windows))]
    pub fn set_user_group(&self, item: impl AsRef<Path>, meta: &Metadata) -> RusticResult<()> {
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
        fchownat(None, &filename, uid, gid, FchownatFlags::NoFollowSymlink)
            .map_err(LocalErrorKind::FromErrnoError)?;
        Ok(())
    }

    #[cfg(windows)]
    // TODO: Windows support
    pub fn set_uid_gid(&self, _item: impl AsRef<Path>, _meta: &Metadata) -> RusticResult<()> {
        Ok(())
    }

    #[cfg(not(windows))]
    pub fn set_uid_gid(&self, item: impl AsRef<Path>, meta: &Metadata) -> RusticResult<()> {
        let filename = self.path(item);

        let uid = meta.uid.map(Uid::from_raw);
        let gid = meta.gid.map(Gid::from_raw);

        fchownat(None, &filename, uid, gid, FchownatFlags::NoFollowSymlink)
            .map_err(LocalErrorKind::FromErrnoError)?;
        Ok(())
    }

    #[cfg(windows)]
    // TODO: Windows support
    pub fn set_permission(&self, _item: impl AsRef<Path>, _node: &Node) -> RusticResult<()> {
        Ok(())
    }

    #[cfg(not(windows))]
    pub fn set_permission(&self, item: impl AsRef<Path>, node: &Node) -> RusticResult<()> {
        if node.node_type.is_symlink() {
            return Ok(());
        }

        let filename = self.path(item);

        if let Some(mode) = node.meta.mode {
            let mode = map_mode_from_go(mode);
            std::fs::set_permissions(filename, fs::Permissions::from_mode(mode))
                .map_err(LocalErrorKind::SettingFilePermissionsFailed)?;
        }
        Ok(())
    }

    #[cfg(any(windows, target_os = "openbsd"))]
    // TODO: Windows support
    // TODO: openbsd support
    pub fn set_extended_attributes(
        &self,
        _item: impl AsRef<Path>,
        _extended_attributes: &[ExtendedAttribute],
    ) -> RusticResult<()> {
        Ok(())
    }

    #[cfg(not(any(windows, target_os = "openbsd")))]
    pub fn set_extended_attributes(
        &self,
        item: impl AsRef<Path>,
        extended_attributes: &[ExtendedAttribute],
    ) -> RusticResult<()> {
        let filename = self.path(item);
        let mut done = vec![false; extended_attributes.len()];

        for curr_name in xattr::list(&filename)
            .map_err(|err| LocalErrorKind::ListingXattrsFailed(err, filename.clone()))?
        {
            match extended_attributes.iter().enumerate().find(
                |(_, ExtendedAttribute { name, .. })| name == curr_name.to_string_lossy().as_ref(),
            ) {
                Some((index, ExtendedAttribute { name, value })) => {
                    let curr_value = xattr::get(&filename, name)
                        .map_err(|err| LocalErrorKind::GettingXattrFailed {
                            name: name.clone(),
                            filename: filename.clone(),
                            source: err,
                        })?
                        .unwrap();
                    if value != &curr_value {
                        xattr::set(&filename, name, value).map_err(|err| {
                            LocalErrorKind::SettingXattrFailed {
                                name: name.clone(),
                                filename: filename.clone(),
                                source: err,
                            }
                        })?;
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
                xattr::set(&filename, name, value).map_err(|err| {
                    LocalErrorKind::SettingXattrFailed {
                        name: name.clone(),
                        filename: filename.clone(),
                        source: err,
                    }
                })?;
            }
        }

        Ok(())
    }

    // set_length sets the length of the given file. If it doesn't exist, create a new (empty) one with given length
    pub fn set_length(&self, item: impl AsRef<Path>, size: u64) -> RusticResult<()> {
        let filename = self.path(item);
        let dir = filename
            .parent()
            .ok_or_else(|| LocalErrorKind::FileDoesNotHaveParent(filename.clone()))?;
        fs::create_dir_all(dir).map_err(LocalErrorKind::DirectoryCreationFailed)?;

        OpenOptions::new()
            .create(true)
            .write(true)
            .open(filename)
            .map_err(LocalErrorKind::OpeningFileFailed)?
            .set_len(size)
            .map_err(LocalErrorKind::SettingFileLengthFailed)?;
        Ok(())
    }

    #[cfg(windows)]
    // TODO: Windows support
    pub fn create_special(&self, _item: impl AsRef<Path>, _node: &Node) -> RusticResult<()> {
        Ok(())
    }

    #[cfg(not(windows))]
    pub fn create_special(&self, item: impl AsRef<Path>, node: &Node) -> RusticResult<()> {
        let filename = self.path(item);

        match &node.node_type {
            NodeType::Symlink { linktarget } => symlink(linktarget.clone(), filename.clone())
                .map_err(|err| LocalErrorKind::SymlinkingFailed {
                    linktarget: linktarget.to_string(),
                    filename,
                    source: err,
                })?,
            NodeType::Dev { device } => {
                #[cfg(not(any(
                    target_os = "macos",
                    target_os = "openbsd",
                    target_os = "freebsd"
                )))]
                let device = *device;
                #[cfg(any(target_os = "macos", target_os = "openbsd"))]
                let device = i32::try_from(*device).map_err(LocalErrorKind::FromTryIntError)?;
                #[cfg(target_os = "freebsd")]
                let device = u32::try_from(*device).map_err(LocalErrorKind::FromTryIntError)?;
                mknod(&filename, SFlag::S_IFBLK, Mode::empty(), device)
                    .map_err(LocalErrorKind::FromErrnoError)?;
            }
            NodeType::Chardev { device } => {
                #[cfg(not(any(
                    target_os = "macos",
                    target_os = "openbsd",
                    target_os = "freebsd"
                )))]
                let device = *device;
                #[cfg(any(target_os = "macos", target_os = "openbsd"))]
                let device = i32::try_from(*device).map_err(LocalErrorKind::FromTryIntError)?;
                #[cfg(target_os = "freebsd")]
                let device = u32::try_from(*device).map_err(LocalErrorKind::FromTryIntError)?;
                mknod(&filename, SFlag::S_IFCHR, Mode::empty(), device)
                    .map_err(LocalErrorKind::FromErrnoError)?;
            }
            NodeType::Fifo => {
                mknod(&filename, SFlag::S_IFIFO, Mode::empty(), 0)
                    .map_err(LocalErrorKind::FromErrnoError)?;
            }
            NodeType::Socket => {
                mknod(&filename, SFlag::S_IFSOCK, Mode::empty(), 0)
                    .map_err(LocalErrorKind::FromErrnoError)?;
            }
            _ => {}
        }
        Ok(())
    }

    pub fn read_at(&self, item: impl AsRef<Path>, offset: u64, length: u64) -> RusticResult<Bytes> {
        let filename = self.path(item);
        let mut file = File::open(filename).map_err(LocalErrorKind::OpeningFileFailed)?;
        _ = file
            .seek(SeekFrom::Start(offset))
            .map_err(LocalErrorKind::CouldNotSeekToPositionInFile)?;
        let mut vec = vec![0; length.try_into().map_err(LocalErrorKind::FromTryIntError)?];
        file.read_exact(&mut vec)
            .map_err(LocalErrorKind::ReadingExactLengthOfFileFailed)?;
        Ok(vec.into())
    }

    pub fn get_matching_file(&self, item: impl AsRef<Path>, size: u64) -> Option<File> {
        let filename = self.path(item);
        fs::symlink_metadata(&filename).map_or_else(
            |_| None,
            |meta| {
                if meta.is_file() && meta.len() == size {
                    File::open(&filename).ok()
                } else {
                    None
                }
            },
        )
    }

    pub fn write_at(&self, item: impl AsRef<Path>, offset: u64, data: &[u8]) -> RusticResult<()> {
        let filename = self.path(item);
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(filename)
            .map_err(LocalErrorKind::OpeningFileFailed)?;
        _ = file
            .seek(SeekFrom::Start(offset))
            .map_err(LocalErrorKind::CouldNotSeekToPositionInFile)?;
        file.write_all(data)
            .map_err(LocalErrorKind::CouldNotWriteToBuffer)?;
        Ok(())
    }
}
