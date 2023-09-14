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
use shell_words::split;
use walkdir::WalkDir;

#[cfg(not(windows))]
use crate::backend::ignore::mapper::map_mode_from_go;
#[cfg(not(windows))]
use crate::backend::node::NodeType;

use crate::{
    backend::{
        node::{ExtendedAttribute, Metadata, Node},
        FileType, ReadBackend, WriteBackend, ALL_FILE_TYPES,
    },
    error::{LocalErrorKind, RusticResult},
    id::Id,
};

/// Local backend, used when backing up.
///
/// This backend is used when backing up to a local directory.
/// It will create a directory structure like this:
///
/// ```text
/// <path>/
/// ├── config
/// ├── data
/// │   ├── 00
/// │   │   └── <id>
/// │   ├── 01
/// │   │   └── <id>
/// │   └── ...
/// ├── index
/// │   └── <id>
/// ├── keys
/// │   └── <id>
/// ├── snapshots
/// │   └── <id>
/// └── ...
/// ```
///
/// The `data` directory will contain all data files, split into 256 subdirectories.
/// The `config` directory will contain the config file.
/// The `index` directory will contain the index file.
/// The `keys` directory will contain the keys file.
/// The `snapshots` directory will contain the snapshots file.
/// All other directories will contain the pack files.
#[derive(Clone, Debug)]
pub struct LocalBackend {
    /// The base path of the backend.
    path: PathBuf,
    /// The command to call after a file was created.
    post_create_command: Option<String>,
    /// The command to call after a file was deleted.
    post_delete_command: Option<String>,
}

impl LocalBackend {
    /// Create a new [`LocalBackend`]
    ///
    /// # Arguments
    ///
    /// * `path` - The base path of the backend
    ///
    /// # Errors
    ///
    /// * [`LocalErrorKind::DirectoryCreationFailed`] - If the directory could not be created.
    ///
    /// [`LocalErrorKind::DirectoryCreationFailed`]: crate::error::LocalErrorKind::DirectoryCreationFailed
    // TODO: We should use `impl Into<Path/PathBuf>` here. we even use it in the body!
    pub fn new(path: &str) -> RusticResult<Self> {
        let path = path.into();
        fs::create_dir_all(&path).map_err(LocalErrorKind::DirectoryCreationFailed)?;
        Ok(Self {
            path,
            post_create_command: None,
            post_delete_command: None,
        })
    }

    /// Path to the given file type and id.
    ///
    /// If the file type is `FileType::Pack`, the id will be used to determine the subdirectory.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `id` - The id of the file.
    ///
    /// # Returns
    ///
    /// The path to the file.
    fn path(&self, tpe: FileType, id: &Id) -> PathBuf {
        let hex_id = id.to_hex();
        match tpe {
            FileType::Config => self.path.join("config"),
            FileType::Pack => self.path.join("data").join(&hex_id[0..2]).join(hex_id),
            _ => self.path.join(tpe.dirname()).join(hex_id),
        }
    }

    /// Call the given command.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `id` - The id of the file.
    /// * `filename` - The path to the file.
    /// * `command` - The command to call.
    ///
    /// # Errors
    ///
    /// * [`LocalErrorKind::FromAhoCorasick`] - If the patterns could not be compiled.
    /// * [`LocalErrorKind::FromSplitError`] - If the command could not be parsed.
    /// * [`LocalErrorKind::CommandExecutionFailed`] - If the command could not be executed.
    /// * [`LocalErrorKind::CommandNotSuccessful`] - If the command was not successful.
    ///
    /// # Notes
    ///
    /// The following placeholders are supported:
    /// * `%file` - The path to the file.
    /// * `%type` - The type of the file.
    /// * `%id` - The id of the file.
    ///
    /// [`LocalErrorKind::FromAhoCorasick`]: crate::error::LocalErrorKind::FromAhoCorasick
    /// [`LocalErrorKind::FromSplitError`]: crate::error::LocalErrorKind::FromSplitError
    /// [`LocalErrorKind::CommandExecutionFailed`]: crate::error::LocalErrorKind::CommandExecutionFailed
    /// [`LocalErrorKind::CommandNotSuccessful`]: crate::error::LocalErrorKind::CommandNotSuccessful
    fn call_command(tpe: FileType, id: &Id, filename: &Path, command: &str) -> RusticResult<()> {
        let id = id.to_hex();
        let patterns = &["%file", "%type", "%id"];
        let ac = AhoCorasick::new(patterns).map_err(LocalErrorKind::FromAhoCorasick)?;
        let replace_with = &[filename.to_str().unwrap(), tpe.dirname(), id.as_str()];
        let actual_command = ac.replace_all(command, replace_with);
        debug!("calling {actual_command}...");
        let commands = split(&actual_command).map_err(LocalErrorKind::FromSplitError)?;
        let status = Command::new(&commands[0])
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
    /// Returns the location of the backend.
    ///
    /// This is `local:<path>`.
    fn location(&self) -> String {
        let mut location = "local:".to_string();
        location.push_str(&self.path.to_string_lossy());
        location
    }

    /// Sets an option of the backend.
    ///
    /// # Arguments
    ///
    /// * `option` - The option to set.
    /// * `value` - The value to set the option to.
    ///
    /// # Notes
    ///
    /// The following options are supported:
    /// * `post-create-command` - The command to call after a file was created.
    /// * `post-delete-command` - The command to call after a file was deleted.
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

    /// Lists all files of the given type.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the files to list.
    ///
    /// # Errors
    ///
    /// * [`IdErrorKind::HexError`] - If the string is not a valid hexadecimal string
    ///
    /// # Notes
    ///
    /// If the file type is `FileType::Config`, this will return a list with a single default id.
    ///
    /// [`IdErrorKind::HexError`]: crate::error::IdErrorKind::HexError
    fn list(&self, tpe: FileType) -> RusticResult<Vec<Id>> {
        trace!("listing tpe: {tpe:?}");
        if tpe == FileType::Config {
            return Ok(if self.path.join("config").exists() {
                vec![Id::default()]
            } else {
                Vec::new()
            });
        }

        let walker = WalkDir::new(self.path.join(tpe.dirname()))
            .into_iter()
            .filter_map(walkdir::Result::ok)
            .filter(|e| e.file_type().is_file())
            .map(|e| Id::from_hex(&e.file_name().to_string_lossy()))
            .filter_map(std::result::Result::ok);
        Ok(walker.collect())
    }

    /// Lists all files with their size of the given type.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the files to list.
    ///
    /// # Errors
    ///
    /// * [`LocalErrorKind::QueryingMetadataFailed`] - If the metadata of the file could not be queried.
    /// * [`LocalErrorKind::FromTryIntError`] - If the length of the file could not be converted to u32.
    /// * [`LocalErrorKind::QueryingWalkDirMetadataFailed`] - If the metadata of the file could not be queried.
    /// * [`IdErrorKind::HexError`] - If the string is not a valid hexadecimal string
    ///
    /// [`LocalErrorKind::QueryingMetadataFailed`]: crate::error::LocalErrorKind::QueryingMetadataFailed
    /// [`LocalErrorKind::FromTryIntError`]: crate::error::LocalErrorKind::FromTryIntError
    /// [`LocalErrorKind::QueryingWalkDirMetadataFailed`]: crate::error::LocalErrorKind::QueryingWalkDirMetadataFailed
    /// [`IdErrorKind::HexError`]: crate::error::IdErrorKind::HexError
    fn list_with_size(&self, tpe: FileType) -> RusticResult<Vec<(Id, u32)>> {
        trace!("listing tpe: {tpe:?}");
        let path = self.path.join(tpe.dirname());

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

    /// Reads full data of the given file.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `id` - The id of the file.
    ///
    /// # Errors
    ///
    /// * [`LocalErrorKind::ReadingContentsOfFileFailed`] - If the file could not be read.
    ///
    /// [`LocalErrorKind::ReadingContentsOfFileFailed`]: crate::error::LocalErrorKind::ReadingContentsOfFileFailed
    fn read_full(&self, tpe: FileType, id: &Id) -> RusticResult<Bytes> {
        trace!("reading tpe: {tpe:?}, id: {id}");
        Ok(fs::read(self.path(tpe, id))
            .map_err(LocalErrorKind::ReadingContentsOfFileFailed)?
            .into())
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
    /// * [`LocalErrorKind::OpeningFileFailed`] - If the file could not be opened.
    /// * [`LocalErrorKind::CouldNotSeekToPositionInFile`] - If the file could not be seeked to the given position.
    /// * [`LocalErrorKind::FromTryIntError`] - If the length of the file could not be converted to u32.
    /// * [`LocalErrorKind::ReadingExactLengthOfFileFailed`] - If the length of the file could not be read.
    ///
    /// [`LocalErrorKind::OpeningFileFailed`]: crate::error::LocalErrorKind::OpeningFileFailed
    /// [`LocalErrorKind::CouldNotSeekToPositionInFile`]: crate::error::LocalErrorKind::CouldNotSeekToPositionInFile
    /// [`LocalErrorKind::FromTryIntError`]: crate::error::LocalErrorKind::FromTryIntError
    /// [`LocalErrorKind::ReadingExactLengthOfFileFailed`]: crate::error::LocalErrorKind::ReadingExactLengthOfFileFailed
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
    /// Create a repository on the backend.
    ///
    /// # Errors
    ///
    /// * [`LocalErrorKind::DirectoryCreationFailed`] - If the directory could not be created.
    ///
    /// [`LocalErrorKind::DirectoryCreationFailed`]: crate::error::LocalErrorKind::DirectoryCreationFailed
    fn create(&self) -> RusticResult<()> {
        trace!("creating repo at {:?}", self.path);

        for tpe in ALL_FILE_TYPES {
            fs::create_dir_all(self.path.join(tpe.dirname()))
                .map_err(LocalErrorKind::DirectoryCreationFailed)?;
        }
        for i in 0u8..=255 {
            fs::create_dir_all(self.path.join("data").join(hex::encode([i])))
                .map_err(LocalErrorKind::DirectoryCreationFailed)?;
        }
        Ok(())
    }

    /// Write the given bytes to the given file.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `id` - The id of the file.
    /// * `cacheable` - Whether the file is cacheable.
    /// * `buf` - The bytes to write.
    ///
    /// # Errors
    ///
    /// * [`LocalErrorKind::OpeningFileFailed`] - If the file could not be opened.
    /// * [`LocalErrorKind::FromTryIntError`] - If the length of the bytes could not be converted to u64.
    /// * [`LocalErrorKind::SettingFileLengthFailed`] - If the length of the file could not be set.
    /// * [`LocalErrorKind::CouldNotWriteToBuffer`] - If the bytes could not be written to the file.
    /// * [`LocalErrorKind::SyncingOfOsMetadataFailed`] - If the metadata of the file could not be synced.
    ///
    /// [`LocalErrorKind::OpeningFileFailed`]: crate::error::LocalErrorKind::OpeningFileFailed
    /// [`LocalErrorKind::FromTryIntError`]: crate::error::LocalErrorKind::FromTryIntError
    /// [`LocalErrorKind::SettingFileLengthFailed`]: crate::error::LocalErrorKind::SettingFileLengthFailed
    /// [`LocalErrorKind::CouldNotWriteToBuffer`]: crate::error::LocalErrorKind::CouldNotWriteToBuffer
    /// [`LocalErrorKind::SyncingOfOsMetadataFailed`]: crate::error::LocalErrorKind::SyncingOfOsMetadataFailed
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

    /// Remove the given file.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `id` - The id of the file.
    /// * `cacheable` - Whether the file is cacheable.
    ///
    /// # Errors
    ///
    /// * [`LocalErrorKind::FileRemovalFailed`] - If the file could not be removed.
    ///
    /// [`LocalErrorKind::FileRemovalFailed`]: crate::error::LocalErrorKind::FileRemovalFailed
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
/// Local destination, used when restoring.
pub struct LocalDestination {
    /// The base path of the destination.
    path: PathBuf,
    /// Whether we expect a single file as destination.
    is_file: bool,
}

impl LocalDestination {
    /// Create a new [`LocalDestination`]
    ///
    /// # Arguments
    ///
    /// * `path` - The base path of the destination
    /// * `create` - If `create` is true, create the base path if it doesn't exist.
    /// * `expect_file` - Whether we expect a single file as destination.
    ///
    /// # Errors
    ///
    /// * [`LocalErrorKind::DirectoryCreationFailed`] - If the directory could not be created.
    ///
    /// [`LocalErrorKind::DirectoryCreationFailed`]: crate::error::LocalErrorKind::DirectoryCreationFailed
    // TODO: We should use `impl Into<Path/PathBuf>` here. we even use it in the body!
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

    /// Path to the given item (relative to the base path)
    ///
    /// # Arguments
    ///
    /// * `item` - The item to get the path for
    ///
    /// # Returns
    ///
    /// The path to the item.
    ///
    /// # Notes
    ///
    /// * If the destination is a file, this will return the base path.
    /// * If the destination is a directory, this will return the base path joined with the item.
    pub(crate) fn path(&self, item: impl AsRef<Path>) -> PathBuf {
        if self.is_file {
            self.path.clone()
        } else {
            self.path.join(item)
        }
    }

    /// Remove the given directory (relative to the base path)
    ///
    /// # Arguments
    ///
    /// * `dirname` - The directory to remove
    ///
    /// # Errors
    ///
    /// * [`LocalErrorKind::DirectoryRemovalFailed`] - If the directory could not be removed.
    ///
    /// # Notes
    ///
    /// This will remove the directory recursively.
    ///
    /// [`LocalErrorKind::DirectoryRemovalFailed`]: crate::error::LocalErrorKind::DirectoryRemovalFailed
    pub fn remove_dir(&self, dirname: impl AsRef<Path>) -> RusticResult<()> {
        Ok(fs::remove_dir_all(dirname).map_err(LocalErrorKind::DirectoryRemovalFailed)?)
    }

    /// Remove the given file (relative to the base path)
    ///
    /// # Arguments
    ///
    /// * `filename` - The file to remove
    ///
    /// # Errors
    ///
    /// * [`LocalErrorKind::FileRemovalFailed`] - If the file could not be removed.
    ///
    /// # Notes
    ///
    /// This will remove the file.
    ///
    /// * If the file is a symlink, the symlink will be removed, not the file it points to.
    /// * If the file is a directory or device, this will fail.
    ///
    /// [`LocalErrorKind::FileRemovalFailed`]: crate::error::LocalErrorKind::FileRemovalFailed
    pub fn remove_file(&self, filename: impl AsRef<Path>) -> RusticResult<()> {
        Ok(fs::remove_file(filename).map_err(LocalErrorKind::FileRemovalFailed)?)
    }

    /// Create the given directory (relative to the base path)
    ///
    /// # Arguments
    ///
    /// * `item` - The directory to create
    ///
    /// # Errors
    ///
    /// * [`LocalErrorKind::DirectoryCreationFailed`] - If the directory could not be created.
    ///
    /// # Notes
    ///
    /// This will create the directory structure recursively.
    ///
    /// [`LocalErrorKind::DirectoryCreationFailed`]: crate::error::LocalErrorKind::DirectoryCreationFailed
    pub fn create_dir(&self, item: impl AsRef<Path>) -> RusticResult<()> {
        let dirname = self.path.join(item);
        fs::create_dir_all(dirname).map_err(LocalErrorKind::DirectoryCreationFailed)?;
        Ok(())
    }

    /// Set changed and modified times for `item` (relative to the base path) utilizing the file metadata
    ///
    /// # Arguments
    ///
    /// * `item` - The item to set the times for
    /// * `meta` - The metadata to get the times from
    ///
    /// # Errors
    ///
    /// * [`LocalErrorKind::SettingTimeMetadataFailed`] - If the times could not be set
    ///
    /// [`LocalErrorKind::SettingTimeMetadataFailed`]: crate::error::LocalErrorKind::SettingTimeMetadataFailed
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
    /// Set user/group for `item` (relative to the base path) utilizing the file metadata
    ///
    /// # Arguments
    ///
    /// * `item` - The item to set the user/group for
    /// * `meta` - The metadata to get the user/group from
    ///
    /// # Errors
    ///
    /// If the user/group could not be set.
    pub fn set_user_group(&self, _item: impl AsRef<Path>, _meta: &Metadata) -> RusticResult<()> {
        // https://learn.microsoft.com/en-us/windows/win32/fileio/file-security-and-access-rights
        // https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Security/struct.SECURITY_ATTRIBUTES.html
        // https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Storage/FileSystem/struct.CREATEFILE2_EXTENDED_PARAMETERS.html#structfield.lpSecurityAttributes
        Ok(())
    }

    #[cfg(not(windows))]
    /// Set user/group for `item` (relative to the base path) utilizing the file metadata
    ///
    /// # Arguments
    ///
    /// * `item` - The item to set the user/group for
    /// * `meta` - The metadata to get the user/group from
    ///
    /// # Errors
    ///
    /// * [`LocalErrorKind::FromErrnoError`] - If the user/group could not be set.
    ///
    /// [`LocalErrorKind::FromErrnoError`]: crate::error::LocalErrorKind::FromErrnoError
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
    /// Set uid/gid for `item` (relative to the base path) utilizing the file metadata
    ///
    /// # Arguments
    ///
    /// * `item` - The item to set the uid/gid for
    /// * `meta` - The metadata to get the uid/gid from
    ///
    /// # Errors
    ///
    /// If the uid/gid could not be set.
    pub fn set_uid_gid(&self, _item: impl AsRef<Path>, _meta: &Metadata) -> RusticResult<()> {
        Ok(())
    }

    #[cfg(not(windows))]
    /// Set uid/gid for `item` (relative to the base path) utilizing the file metadata
    ///
    /// # Arguments
    ///
    /// * `item` - The item to set the uid/gid for
    /// * `meta` - The metadata to get the uid/gid from
    ///
    /// # Errors
    ///
    /// * [`LocalErrorKind::FromErrnoError`] - If the uid/gid could not be set.
    ///
    /// [`LocalErrorKind::FromErrnoError`]: crate::error::LocalErrorKind::FromErrnoError
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
    /// Set permissions for `item` (relative to the base path) from `node`
    ///
    /// # Arguments
    ///
    /// * `item` - The item to set the permissions for
    /// * `node` - The node to get the permissions from
    ///
    /// # Errors        
    ///
    /// If the permissions could not be set.
    pub fn set_permission(&self, _item: impl AsRef<Path>, _node: &Node) -> RusticResult<()> {
        Ok(())
    }

    #[cfg(not(windows))]
    /// Set permissions for `item` (relative to the base path) from `node`
    ///
    /// # Arguments
    ///
    /// * `item` - The item to set the permissions for
    /// * `node` - The node to get the permissions from
    ///
    /// # Errors        
    ///
    /// * [`LocalErrorKind::SettingFilePermissionsFailed`] - If the permissions could not be set.
    ///
    /// [`LocalErrorKind::SettingFilePermissionsFailed`]: crate::error::LocalErrorKind::SettingFilePermissionsFailed
    pub fn set_permission(&self, item: impl AsRef<Path>, node: &Node) -> RusticResult<()> {
        if node.is_symlink() {
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
    /// Set extended attributes for `item` (relative to the base path)
    ///
    /// # Arguments
    ///
    /// * `item` - The item to set the extended attributes for
    /// * `extended_attributes` - The extended attributes to set
    ///
    /// # Errors
    ///
    /// If the extended attributes could not be set.
    pub fn set_extended_attributes(
        &self,
        _item: impl AsRef<Path>,
        _extended_attributes: &[ExtendedAttribute],
    ) -> RusticResult<()> {
        Ok(())
    }

    #[cfg(not(any(windows, target_os = "openbsd")))]
    /// Set extended attributes for `item` (relative to the base path)
    ///
    /// # Arguments
    ///
    /// * `item` - The item to set the extended attributes for
    /// * `extended_attributes` - The extended attributes to set
    ///
    /// # Errors
    ///
    /// * [`LocalErrorKind::ListingXattrsFailed`] - If listing the extended attributes failed.
    /// * [`LocalErrorKind::GettingXattrFailed`] - If getting an extended attribute failed.
    /// * [`LocalErrorKind::SettingXattrFailed`] - If setting an extended attribute failed.
    ///
    /// [`LocalErrorKind::ListingXattrsFailed`]: crate::error::LocalErrorKind::ListingXattrsFailed
    /// [`LocalErrorKind::GettingXattrFailed`]: crate::error::LocalErrorKind::GettingXattrFailed
    /// [`LocalErrorKind::SettingXattrFailed`]: crate::error::LocalErrorKind::SettingXattrFailed
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

    /// Set length of `item` (relative to the base path)
    ///
    /// # Arguments
    ///
    /// * `item` - The item to set the length for
    /// * `size` - The size to set the length to
    ///
    /// # Errors
    ///
    /// * [`LocalErrorKind::FileDoesNotHaveParent`] - If the file does not have a parent.
    /// * [`LocalErrorKind::DirectoryCreationFailed`] - If the directory could not be created.
    /// * [`LocalErrorKind::OpeningFileFailed`] - If the file could not be opened.
    /// * [`LocalErrorKind::SettingFileLengthFailed`] - If the length of the file could not be set.
    ///
    /// # Notes
    ///
    /// If the file exists, truncate it to the given length. (TODO: check if this is correct)
    /// If it doesn't exist, create a new (empty) one with given length.
    ///
    /// [`LocalErrorKind::FileDoesNotHaveParent`]: crate::error::LocalErrorKind::FileDoesNotHaveParent
    /// [`LocalErrorKind::DirectoryCreationFailed`]: crate::error::LocalErrorKind::DirectoryCreationFailed
    /// [`LocalErrorKind::OpeningFileFailed`]: crate::error::LocalErrorKind::OpeningFileFailed
    /// [`LocalErrorKind::SettingFileLengthFailed`]: crate::error::LocalErrorKind::SettingFileLengthFailed
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
    /// Create a special file (relative to the base path)
    pub fn create_special(&self, _item: impl AsRef<Path>, _node: &Node) -> RusticResult<()> {
        Ok(())
    }

    #[cfg(not(windows))]
    /// Create a special file (relative to the base path)
    ///
    /// # Arguments
    ///
    /// * `item` - The item to create
    /// * `node` - The node to get the type from
    ///
    /// # Errors
    ///
    /// * [`LocalErrorKind::SymlinkingFailed`] - If the symlink could not be created.
    /// * [`LocalErrorKind::FromTryIntError`] - If the device could not be converted to the correct type.
    /// * [`LocalErrorKind::FromErrnoError`] - If the device could not be created.
    ///
    /// [`LocalErrorKind::SymlinkingFailed`]: crate::error::LocalErrorKind::SymlinkingFailed
    /// [`LocalErrorKind::FromTryIntError`]: crate::error::LocalErrorKind::FromTryIntError
    /// [`LocalErrorKind::FromErrnoError`]: crate::error::LocalErrorKind::FromErrnoError
    pub fn create_special(&self, item: impl AsRef<Path>, node: &Node) -> RusticResult<()> {
        let filename = self.path(item);

        match &node.node_type {
            NodeType::Symlink { .. } => {
                let linktarget = node.node_type.to_link();
                symlink(linktarget, &filename).map_err(|err| LocalErrorKind::SymlinkingFailed {
                    linktarget: linktarget.to_path_buf(),
                    filename,
                    source: err,
                })?;
            }
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

    /// Read the given item (relative to the base path)
    ///
    /// # Arguments
    ///
    /// * `item` - The item to read
    /// * `offset` - The offset to read from
    /// * `length` - The length to read
    ///
    /// # Errors
    ///
    /// * [`LocalErrorKind::OpeningFileFailed`] - If the file could not be opened.
    /// * [`LocalErrorKind::CouldNotSeekToPositionInFile`] - If the file could not be seeked to the given position.
    /// * [`LocalErrorKind::FromTryIntError`] - If the length of the file could not be converted to u32.
    /// * [`LocalErrorKind::ReadingExactLengthOfFileFailed`] - If the length of the file could not be read.
    ///
    /// [`LocalErrorKind::OpeningFileFailed`]: crate::error::LocalErrorKind::OpeningFileFailed
    /// [`LocalErrorKind::CouldNotSeekToPositionInFile`]: crate::error::LocalErrorKind::CouldNotSeekToPositionInFile
    /// [`LocalErrorKind::FromTryIntError`]: crate::error::LocalErrorKind::FromTryIntError
    /// [`LocalErrorKind::ReadingExactLengthOfFileFailed`]: crate::error::LocalErrorKind::ReadingExactLengthOfFileFailed
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

    /// Check if a matching file exists.
    ///
    /// # Arguments
    ///
    /// * `item` - The item to check
    /// * `size` - The size to check
    ///
    /// # Returns
    ///
    /// If a file exists and size matches, this returns a `File` open for reading.
    /// In all other cases, returns `None`
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

    /// Write `data` to given item (relative to the base path) at `offset`
    ///
    /// # Arguments
    ///
    /// * `item` - The item to write to
    /// * `offset` - The offset to write at
    /// * `data` - The data to write
    ///
    /// # Errors
    ///
    /// * [`LocalErrorKind::OpeningFileFailed`] - If the file could not be opened.
    /// * [`LocalErrorKind::CouldNotSeekToPositionInFile`] - If the file could not be seeked to the given position.
    /// * [`LocalErrorKind::CouldNotWriteToBuffer`] - If the bytes could not be written to the file.
    ///
    /// # Notes
    ///
    /// This will create the file if it doesn't exist.
    ///
    /// [`LocalErrorKind::OpeningFileFailed`]: crate::error::LocalErrorKind::OpeningFileFailed
    /// [`LocalErrorKind::CouldNotSeekToPositionInFile`]: crate::error::LocalErrorKind::CouldNotSeekToPositionInFile
    /// [`LocalErrorKind::CouldNotWriteToBuffer`]: crate::error::LocalErrorKind::CouldNotWriteToBuffer
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
