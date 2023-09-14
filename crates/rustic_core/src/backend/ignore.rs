#[cfg(not(windows))]
use std::os::unix::fs::{FileTypeExt, MetadataExt};

use std::{
    fs::{read_link, File},
    path::{Path, PathBuf},
};

use serde_with::{serde_as, DisplayFromStr};

use bytesize::ByteSize;
#[cfg(not(windows))]
use cached::proc_macro::cached;
#[cfg(not(windows))]
use chrono::TimeZone;
use chrono::{DateTime, Local, Utc};
use derive_setters::Setters;
use ignore::{overrides::OverrideBuilder, DirEntry, Walk, WalkBuilder};
use log::warn;
#[cfg(not(windows))]
use nix::unistd::{Gid, Group, Uid, User};

#[cfg(not(any(windows, target_os = "openbsd")))]
use crate::backend::node::ExtendedAttribute;

use crate::{
    backend::{
        node::{Metadata, Node, NodeType},
        ReadSource, ReadSourceEntry, ReadSourceOpen,
    },
    error::{IgnoreErrorKind, RusticResult},
};

// Walk doesn't implement Debug
#[allow(missing_debug_implementations)]
/// A [`LocalSource`] is a source from local paths which is used to be read from (i.e. to backup it).
pub struct LocalSource {
    /// The walk builder.
    builder: WalkBuilder,
    /// The walk iterator.
    walker: Walk,
    /// The save options to use.
    save_opts: LocalSourceSaveOptions,
}

#[serde_as]
#[cfg_attr(feature = "clap", derive(clap::Parser))]
#[cfg_attr(feature = "merge", derive(merge::Merge))]
#[derive(serde::Deserialize, serde::Serialize, Default, Clone, Copy, Debug, Setters)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
#[setters(into)]
/// [`LocalSourceSaveOptions`] describes how entries from a local source will be saved in the repository.
pub struct LocalSourceSaveOptions {
    /// Save access time for files and directories
    #[cfg_attr(feature = "clap", clap(long))]
    #[cfg_attr(feature = "merge", merge(strategy = merge::bool::overwrite_false))]
    pub with_atime: bool,

    /// Don't save device ID for files and directories
    #[cfg_attr(feature = "clap", clap(long))]
    #[cfg_attr(feature = "merge", merge(strategy = merge::bool::overwrite_false))]
    pub ignore_devid: bool,
}

#[serde_as]
#[cfg_attr(feature = "clap", derive(clap::Parser))]
#[cfg_attr(feature = "merge", derive(merge::Merge))]
#[derive(serde::Deserialize, serde::Serialize, Default, Clone, Debug, Setters)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
#[setters(into)]
/// [`LocalSourceFilterOptions`] allow to filter a local source by various criteria.
pub struct LocalSourceFilterOptions {
    /// Glob pattern to exclude/include (can be specified multiple times)
    #[cfg_attr(feature = "clap", clap(long))]
    #[cfg_attr(feature = "merge", merge(strategy = merge::vec::overwrite_empty))]
    pub glob: Vec<String>,

    /// Same as --glob pattern but ignores the casing of filenames
    #[cfg_attr(feature = "clap", clap(long, value_name = "GLOB"))]
    #[cfg_attr(feature = "merge", merge(strategy = merge::vec::overwrite_empty))]
    pub iglob: Vec<String>,

    /// Read glob patterns to exclude/include from this file (can be specified multiple times)
    #[cfg_attr(feature = "clap", clap(long, value_name = "FILE"))]
    #[cfg_attr(feature = "merge", merge(strategy = merge::vec::overwrite_empty))]
    pub glob_file: Vec<String>,

    /// Same as --glob-file ignores the casing of filenames in patterns
    #[cfg_attr(feature = "clap", clap(long, value_name = "FILE"))]
    #[cfg_attr(feature = "merge", merge(strategy = merge::vec::overwrite_empty))]
    pub iglob_file: Vec<String>,

    /// Ignore files based on .gitignore files
    #[cfg_attr(feature = "clap", clap(long))]
    #[cfg_attr(feature = "merge", merge(strategy = merge::bool::overwrite_false))]
    pub git_ignore: bool,

    /// Do not require a git repository to apply git-ignore rule
    #[cfg_attr(feature = "clap", clap(long))]
    #[cfg_attr(feature = "merge", merge(strategy = merge::bool::overwrite_false))]
    pub no_require_git: bool,

    /// Exclude contents of directories containing this filename (can be specified multiple times)
    #[cfg_attr(feature = "clap", clap(long, value_name = "FILE"))]
    #[cfg_attr(feature = "merge", merge(strategy = merge::vec::overwrite_empty))]
    pub exclude_if_present: Vec<String>,

    /// Exclude other file systems, don't cross filesystem boundaries and subvolumes
    #[cfg_attr(feature = "clap", clap(long, short = 'x'))]
    #[cfg_attr(feature = "merge", merge(strategy = merge::bool::overwrite_false))]
    pub one_file_system: bool,

    /// Maximum size of files to be backed up. Larger files will be excluded.
    #[cfg_attr(feature = "clap", clap(long, value_name = "SIZE"))]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub exclude_larger_than: Option<ByteSize>,
}

impl LocalSource {
    /// Create a local source from [`LocalSourceSaveOptions`], [`LocalSourceFilterOptions`] and backup path(s).
    ///
    /// # Arguments
    ///
    /// * `save_opts` - The [`LocalSourceSaveOptions`] to use.
    /// * `filter_opts` - The [`LocalSourceFilterOptions`] to use.
    /// * `backup_paths` - The backup path(s) to use.
    ///
    /// # Returns
    ///
    /// The created local source.
    ///
    /// # Errors
    ///
    /// * [`IgnoreErrorKind::GenericError`] - If the a glob pattern could not be added to the override builder.
    /// * [`IgnoreErrorKind::FromIoError`] - If a glob file could not be read.
    ///
    /// [`IgnoreErrorKind::GenericError`]: crate::error::IgnoreErrorKind::GenericError
    /// [`IgnoreErrorKind::FromIoError`]: crate::error::IgnoreErrorKind::FromIoError
    pub fn new(
        save_opts: LocalSourceSaveOptions,
        filter_opts: &LocalSourceFilterOptions,
        backup_paths: &[impl AsRef<Path>],
    ) -> RusticResult<Self> {
        let mut walk_builder = WalkBuilder::new(&backup_paths[0]);

        for path in &backup_paths[1..] {
            _ = walk_builder.add(path);
        }

        let mut override_builder = OverrideBuilder::new("");

        for g in &filter_opts.glob {
            _ = override_builder
                .add(g)
                .map_err(IgnoreErrorKind::GenericError)?;
        }

        for file in &filter_opts.glob_file {
            for line in std::fs::read_to_string(file)
                .map_err(IgnoreErrorKind::FromIoError)?
                .lines()
            {
                _ = override_builder
                    .add(line)
                    .map_err(IgnoreErrorKind::GenericError)?;
            }
        }

        _ = override_builder
            .case_insensitive(true)
            .map_err(IgnoreErrorKind::GenericError)?;
        for g in &filter_opts.iglob {
            _ = override_builder
                .add(g)
                .map_err(IgnoreErrorKind::GenericError)?;
        }

        for file in &filter_opts.iglob_file {
            for line in std::fs::read_to_string(file)
                .map_err(IgnoreErrorKind::FromIoError)?
                .lines()
            {
                _ = override_builder
                    .add(line)
                    .map_err(IgnoreErrorKind::GenericError)?;
            }
        }

        _ = walk_builder
            .follow_links(false)
            .hidden(false)
            .ignore(false)
            .git_ignore(filter_opts.git_ignore)
            .require_git(!filter_opts.no_require_git)
            .sort_by_file_path(Path::cmp)
            .same_file_system(filter_opts.one_file_system)
            .max_filesize(filter_opts.exclude_larger_than.map(|s| s.as_u64()))
            .overrides(
                override_builder
                    .build()
                    .map_err(IgnoreErrorKind::GenericError)?,
            );

        let exclude_if_present = filter_opts.exclude_if_present.clone();
        if !filter_opts.exclude_if_present.is_empty() {
            _ = walk_builder.filter_entry(move |entry| match entry.file_type() {
                Some(tpe) if tpe.is_dir() => {
                    for file in &exclude_if_present {
                        if entry.path().join(file).exists() {
                            return false;
                        }
                    }
                    true
                }
                _ => true,
            });
        }

        let builder = walk_builder;
        let walker = builder.build();

        Ok(Self {
            builder,
            walker,
            save_opts,
        })
    }
}

#[derive(Debug)]
/// Describes an open file from the local backend.
pub struct OpenFile(PathBuf);

impl ReadSourceOpen for OpenFile {
    type Reader = File;

    /// Open the file from the local backend.
    ///
    /// # Returns
    ///
    /// The read handle to the file from the local backend.
    ///
    /// # Errors
    ///
    /// * [`IgnoreErrorKind::UnableToOpenFile`] - If the file could not be opened.
    ///
    /// [`IgnoreErrorKind::UnableToOpenFile`]: crate::error::IgnoreErrorKind::UnableToOpenFile
    fn open(self) -> RusticResult<Self::Reader> {
        let path = self.0;
        File::open(path).map_err(|err| IgnoreErrorKind::UnableToOpenFile(err).into())
    }
}

impl ReadSource for LocalSource {
    type Open = OpenFile;
    type Iter = Self;

    /// Get the size of the local source.
    ///
    /// # Returns
    ///
    /// The size of the local source or `None` if the size could not be determined.
    ///
    /// # Errors
    ///
    /// If the size could not be determined.
    fn size(&self) -> RusticResult<Option<u64>> {
        let mut size = 0;
        for entry in self.builder.build() {
            if let Err(e) = entry.and_then(|e| e.metadata()).map(|m| {
                size += if m.is_dir() { 0 } else { m.len() };
            }) {
                warn!("ignoring error {}", e);
            }
        }
        Ok(Some(size))
    }

    /// Iterate over the entries of the local source.
    ///
    /// # Returns
    ///
    /// An iterator over the entries of the local source.
    fn entries(self) -> Self::Iter {
        self
    }
}

impl Iterator for LocalSource {
    type Item = RusticResult<ReadSourceEntry<OpenFile>>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.walker.next() {
            // ignore root dir, i.e. an entry with depth 0 of type dir
            Some(Ok(entry)) if entry.depth() == 0 && entry.file_type().unwrap().is_dir() => {
                self.walker.next()
            }
            item => item,
        }
        .map(|e| {
            map_entry(
                e.map_err(IgnoreErrorKind::GenericError)?,
                self.save_opts.with_atime,
                self.save_opts.ignore_devid,
            )
            .map_err(std::convert::Into::into)
        })
    }
}

/// Maps a [`DirEntry`] to a [`ReadSourceEntry`].
///
/// # Arguments
///
/// * `entry` - The [`DirEntry`] to map.
/// * `with_atime` - Whether to save access time for files and directories.
/// * `ignore_devid` - Whether to save device ID for files and directories.
///
/// # Errors
///
/// * [`IgnoreErrorKind::GenericError`] - If metadata could not be read.
/// * [`IgnoreErrorKind::FromIoError`] - If path of the entry could not be read.
///
/// [`IgnoreErrorKind::GenericError`]: crate::error::IgnoreErrorKind::GenericError
/// [`IgnoreErrorKind::FromIoError`]: crate::error::IgnoreErrorKind::FromIoError
#[cfg(windows)]
fn map_entry(
    entry: DirEntry,
    with_atime: bool,
    _ignore_devid: bool,
) -> RusticResult<ReadSourceEntry<OpenFile>> {
    let name = entry.file_name();
    let m = entry.metadata().map_err(IgnoreErrorKind::GenericError)?;

    // TODO: Set them to suitable values
    let uid = None;
    let gid = None;
    let user = None;
    let group = None;

    let size = if m.is_dir() { 0 } else { m.len() };
    let mode = None;
    let inode = 0;
    let device_id = 0;
    let links = 0;

    let mtime = m
        .modified()
        .ok()
        .map(|t| DateTime::<Utc>::from(t).with_timezone(&Local));
    let atime = if with_atime {
        m.accessed()
            .ok()
            .map(|t| DateTime::<Utc>::from(t).with_timezone(&Local))
    } else {
        // TODO: Use None here?
        mtime
    };
    let ctime = m
        .created()
        .ok()
        .map(|t| DateTime::<Utc>::from(t).with_timezone(&Local));

    let meta = Metadata {
        size,
        mtime,
        atime,
        ctime,
        mode,
        uid,
        gid,
        user,
        group,
        inode,
        device_id,
        links,
        extended_attributes: Vec::new(),
    };

    let node = if m.is_dir() {
        Node::new_node(name, NodeType::Dir, meta)
    } else if m.is_symlink() {
        let target = read_link(entry.path()).map_err(IgnoreErrorKind::FromIoError)?;
        let node_type = NodeType::from_link(&target);
        Node::new_node(name, node_type, meta)
    } else {
        Node::new_node(name, NodeType::File, meta)
    };

    let path = entry.into_path();
    let open = Some(OpenFile(path.clone()));
    Ok(ReadSourceEntry { path, node, open })
}

/// Get the user name for the given uid.
///
/// # Arguments
///
/// * `uid` - The uid to get the user name for.
///
/// # Returns
///
/// The user name for the given uid or `None` if the user could not be found.
#[cfg(not(windows))]
#[cached]
fn get_user_by_uid(uid: u32) -> Option<String> {
    match User::from_uid(Uid::from_raw(uid)) {
        Ok(Some(user)) => Some(user.name),
        Ok(None) => None,
        Err(err) => {
            warn!("error getting user from uid {uid}: {err}");
            None
        }
    }
}

/// Get the group name for the given gid.
///
/// # Arguments
///
/// * `gid` - The gid to get the group name for.
///
/// # Returns
///
/// The group name for the given gid or `None` if the group could not be found.
#[cfg(not(windows))]
#[cached]
fn get_group_by_gid(gid: u32) -> Option<String> {
    match Group::from_gid(Gid::from_raw(gid)) {
        Ok(Some(group)) => Some(group.name),
        Ok(None) => None,
        Err(err) => {
            warn!("error getting group from gid {gid}: {err}");
            None
        }
    }
}

/// Maps a [`DirEntry`] to a [`ReadSourceEntry`].
///
/// # Arguments
///
/// * `entry` - The [`DirEntry`] to map.
/// * `with_atime` - Whether to save access time for files and directories.
/// * `ignore_devid` - Whether to save device ID for files and directories.
///
/// # Errors
///
/// * [`IgnoreErrorKind::GenericError`] - If metadata could not be read.
/// * [`IgnoreErrorKind::FromIoError`] - If the xattr of the entry could not be read.
///
/// [`IgnoreErrorKind::GenericError`]: crate::error::IgnoreErrorKind::GenericError
/// [`IgnoreErrorKind::FromIoError`]: crate::error::IgnoreErrorKind::FromIoError
#[cfg(not(windows))]
// map_entry: turn entry into (Path, Node)
fn map_entry(
    entry: DirEntry,
    with_atime: bool,
    ignore_devid: bool,
) -> RusticResult<ReadSourceEntry<OpenFile>> {
    let name = entry.file_name();
    let m = entry.metadata().map_err(IgnoreErrorKind::GenericError)?;

    let uid = m.uid();
    let gid = m.gid();
    let user = get_user_by_uid(uid);
    let group = get_group_by_gid(gid);

    let mtime = m
        .modified()
        .ok()
        .map(|t| DateTime::<Utc>::from(t).with_timezone(&Local));
    let atime = if with_atime {
        m.accessed()
            .ok()
            .map(|t| DateTime::<Utc>::from(t).with_timezone(&Local))
    } else {
        // TODO: Use None here?
        mtime
    };
    let ctime = Utc
        .timestamp_opt(
            m.ctime(),
            m.ctime_nsec()
                .try_into()
                .map_err(IgnoreErrorKind::FromTryFromIntError)?,
        )
        .single()
        .map(|dt| dt.with_timezone(&Local));

    let size = if m.is_dir() { 0 } else { m.len() };
    let mode = mapper::map_mode_to_go(m.mode());
    let inode = m.ino();
    let device_id = if ignore_devid { 0 } else { m.dev() };
    let links = if m.is_dir() { 0 } else { m.nlink() };

    #[cfg(target_os = "openbsd")]
    let extended_attributes = vec![];

    #[cfg(not(target_os = "openbsd"))]
    let extended_attributes = {
        let path = entry.path();
        xattr::list(path)
            .map_err(IgnoreErrorKind::FromIoError)?
            .map(|name| {
                Ok(ExtendedAttribute {
                    name: name.to_string_lossy().to_string(),
                    value: xattr::get(path, name)
                        .map_err(IgnoreErrorKind::FromIoError)?
                        .unwrap(),
                })
            })
            .collect::<RusticResult<_>>()?
    };

    let meta = Metadata {
        size,
        mtime,
        atime,
        ctime,
        mode: Some(mode),
        uid: Some(uid),
        gid: Some(gid),
        user,
        group,
        inode,
        device_id,
        links,
        extended_attributes,
    };
    let filetype = m.file_type();

    let node = if m.is_dir() {
        Node::new_node(name, NodeType::Dir, meta)
    } else if m.is_symlink() {
        let target = read_link(entry.path()).map_err(IgnoreErrorKind::FromIoError)?;
        let node_type = NodeType::from_link(&target);
        Node::new_node(name, node_type, meta)
    } else if filetype.is_block_device() {
        let node_type = NodeType::Dev { device: m.rdev() };
        Node::new_node(name, node_type, meta)
    } else if filetype.is_char_device() {
        let node_type = NodeType::Chardev { device: m.rdev() };
        Node::new_node(name, node_type, meta)
    } else if filetype.is_fifo() {
        Node::new_node(name, NodeType::Fifo, meta)
    } else if filetype.is_socket() {
        Node::new_node(name, NodeType::Socket, meta)
    } else {
        Node::new_node(name, NodeType::File, meta)
    };
    let path = entry.into_path();
    let open = Some(OpenFile(path.clone()));
    Ok(ReadSourceEntry { path, node, open })
}

#[cfg(not(windows))]
pub mod mapper {
    const MODE_PERM: u32 = 0o777; // permission bits

    // consts from https://pkg.go.dev/io/fs#ModeType
    const GO_MODE_DIR: u32 = 0b10000000000000000000000000000000;
    const GO_MODE_SYMLINK: u32 = 0b00001000000000000000000000000000;
    const GO_MODE_DEVICE: u32 = 0b00000100000000000000000000000000;
    const GO_MODE_FIFO: u32 = 0b00000010000000000000000000000000;
    const GO_MODE_SOCKET: u32 = 0b00000001000000000000000000000000;
    const GO_MODE_SETUID: u32 = 0b00000000100000000000000000000000;
    const GO_MODE_SETGID: u32 = 0b00000000010000000000000000000000;
    const GO_MODE_CHARDEV: u32 = 0b00000000001000000000000000000000;
    const GO_MODE_STICKY: u32 = 0b00000000000100000000000000000000;
    const GO_MODE_IRREG: u32 = 0b00000000000010000000000000000000;

    // consts from man page inode(7)
    const S_IFFORMAT: u32 = 0o170000; // File mask
    const S_IFSOCK: u32 = 0o140000; // socket
    const S_IFLNK: u32 = 0o120000; // symbolic link
    const S_IFREG: u32 = 0o100000; // regular file
    const S_IFBLK: u32 = 0o060000; // block device
    const S_IFDIR: u32 = 0o040000; // directory
    const S_IFCHR: u32 = 0o020000; // character device
    const S_IFIFO: u32 = 0o010000; // FIFO

    const S_ISUID: u32 = 0o4000; // set-user-ID bit (see execve(2))
    const S_ISGID: u32 = 0o2000; // set-group-ID bit (see below)
    const S_ISVTX: u32 = 0o1000; // sticky bit (see below)

    /// map `st_mode` from POSIX (`inode(7)`) to golang's definition (<https://pkg.go.dev/io/fs#ModeType>)
    /// Note, that it only sets the bits `os.ModePerm | os.ModeType | os.ModeSetuid | os.ModeSetgid | os.ModeSticky`
    /// to stay compatible with the restic implementation
    pub const fn map_mode_to_go(mode: u32) -> u32 {
        let mut go_mode = mode & MODE_PERM;

        match mode & S_IFFORMAT {
            S_IFSOCK => go_mode |= GO_MODE_SOCKET,
            S_IFLNK => go_mode |= GO_MODE_SYMLINK,
            S_IFBLK => go_mode |= GO_MODE_DEVICE,
            S_IFDIR => go_mode |= GO_MODE_DIR,
            S_IFCHR => go_mode |= GO_MODE_CHARDEV & GO_MODE_DEVICE, // no idea why go sets both for char devices...
            S_IFIFO => go_mode |= GO_MODE_FIFO,
            // note that POSIX specifies regular files, whereas golang specifies irregular files
            S_IFREG => {}
            _ => go_mode |= GO_MODE_IRREG,
        };

        if mode & S_ISUID > 0 {
            go_mode |= GO_MODE_SETUID;
        }
        if mode & S_ISGID > 0 {
            go_mode |= GO_MODE_SETGID;
        }
        if mode & S_ISVTX > 0 {
            go_mode |= GO_MODE_STICKY;
        }

        go_mode
    }

    /// map golangs mode definition (<https://pkg.go.dev/io/fs#ModeType>) to `st_mode` from POSIX (`inode(7)`)
    /// This is the inverse function to [`map_mode_to_go`]
    pub const fn map_mode_from_go(go_mode: u32) -> u32 {
        let mut mode = go_mode & MODE_PERM;

        if go_mode & GO_MODE_SOCKET > 0 {
            mode |= S_IFSOCK;
        } else if go_mode & GO_MODE_SYMLINK > 0 {
            mode |= S_IFLNK;
        } else if go_mode & GO_MODE_DEVICE > 0 && go_mode & GO_MODE_CHARDEV == 0 {
            mode |= S_IFBLK;
        } else if go_mode & GO_MODE_DIR > 0 {
            mode |= S_IFDIR;
        } else if go_mode & (GO_MODE_CHARDEV | GO_MODE_DEVICE) > 0 {
            mode |= S_IFCHR;
        } else if go_mode & GO_MODE_FIFO > 0 {
            mode |= S_IFIFO;
        } else if go_mode & GO_MODE_IRREG > 0 {
            // note that POSIX specifies regular files, whereas golang specifies irregular files
        } else {
            mode |= S_IFREG;
        }

        if go_mode & GO_MODE_SETUID > 0 {
            mode |= S_ISUID;
        }
        if go_mode & GO_MODE_SETGID > 0 {
            mode |= S_ISGID;
        }
        if go_mode & GO_MODE_STICKY > 0 {
            mode |= S_ISVTX;
        }

        mode
    }
}
