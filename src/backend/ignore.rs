use std::fs::{read_link, File};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};

use anyhow::Result;
use bytesize::ByteSize;
use chrono::{Local, TimeZone, Utc};
use clap::Parser;
use ignore::{overrides::OverrideBuilder, DirEntry, Walk, WalkBuilder};
use log::*;
use merge::Merge;
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};
use users::{Groups, Users, UsersCache};

use super::{node::Metadata, node::NodeType, Node, ReadSource};

pub struct LocalSource {
    builder: WalkBuilder,
    walker: Walk,
    with_atime: bool,
    ignore_devid: bool,
    cache: UsersCache,
}

#[serde_as]
#[derive(Default, Clone, Parser, Deserialize, Merge)]
#[serde(default, rename_all = "kebab-case")]
pub struct LocalSourceOptions {
    /// Save access time for files and directories
    #[clap(long)]
    #[merge(strategy = merge::bool::overwrite_false)]
    with_atime: bool,

    /// Don't save device ID for files and directories
    #[clap(long)]
    #[merge(strategy = merge::bool::overwrite_false)]
    ignore_devid: bool,

    /// Glob pattern to exclude/include (can be specified multiple times)
    #[clap(long, short = 'g', help_heading = "EXCLUDE OPTIONS")]
    #[merge(strategy = merge::vec::overwrite_empty)]
    glob: Vec<String>,

    /// Same as --glob pattern but ignores the casing of filenames
    #[clap(long, value_name = "GLOB", help_heading = "EXCLUDE OPTIONS")]
    #[merge(strategy = merge::vec::overwrite_empty)]
    iglob: Vec<String>,

    /// Read glob patterns to exclude/include from this file (can be specified multiple times)
    #[clap(long, value_name = "FILE", help_heading = "EXCLUDE OPTIONS")]
    #[merge(strategy = merge::vec::overwrite_empty)]
    glob_file: Vec<String>,

    /// Same as --glob-file ignores the casing of filenames in patterns
    #[clap(long, value_name = "FILE", help_heading = "EXCLUDE OPTIONS")]
    #[merge(strategy = merge::vec::overwrite_empty)]
    iglob_file: Vec<String>,

    /// Ignore files based on .gitignore files
    #[clap(long, help_heading = "EXCLUDE OPTIONS")]
    #[merge(strategy = merge::bool::overwrite_false)]
    git_ignore: bool,

    /// Exclude contents of directories containing this filename (can be specified multiple times)
    #[clap(long, value_name = "FILE", help_heading = "EXCLUDE OPTIONS")]
    #[merge(strategy = merge::vec::overwrite_empty)]
    exclude_if_present: Vec<String>,

    /// Exclude other file systems, don't cross filesystem boundaries and subvolumes
    #[clap(long, short = 'x', help_heading = "EXCLUDE OPTIONS")]
    #[merge(strategy = merge::bool::overwrite_false)]
    one_file_system: bool,

    /// Maximum size of files to be backuped. Larger files will be excluded.
    #[clap(long, value_name = "SIZE", help_heading = "EXCLUDE OPTIONS")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    exclude_larger_than: Option<ByteSize>,
}

impl LocalSource {
    pub fn new(opts: LocalSourceOptions, backup_path: PathBuf) -> Result<Self> {
        let mut walk_builder = WalkBuilder::new(backup_path);
        /*
         for path in &paths[1..] {
            wb.add(path);
        }
        */

        let mut override_builder = OverrideBuilder::new("/");

        for g in opts.glob {
            override_builder.add(&g)?;
        }

        for file in opts.glob_file {
            for line in std::fs::read_to_string(file)?.lines() {
                override_builder.add(line)?;
            }
        }

        override_builder.case_insensitive(true)?;
        for g in opts.iglob {
            override_builder.add(&g)?;
        }

        for file in opts.iglob_file {
            for line in std::fs::read_to_string(file)?.lines() {
                override_builder.add(line)?;
            }
        }

        walk_builder
            .follow_links(false)
            .hidden(false)
            .ignore(false)
            .git_ignore(opts.git_ignore)
            .sort_by_file_path(Path::cmp)
            .same_file_system(opts.one_file_system)
            .max_filesize(opts.exclude_larger_than.map(|s| s.as_u64()))
            .overrides(override_builder.build()?);

        if !opts.exclude_if_present.is_empty() {
            walk_builder.filter_entry(move |entry| match entry.file_type() {
                None => true,
                Some(tpe) if tpe.is_dir() => {
                    for file in &opts.exclude_if_present {
                        if entry.path().join(file).exists() {
                            return false;
                        }
                    }
                    true
                }
                Some(_) => true,
            });
        }

        let builder = walk_builder;
        let walker = builder.build();

        Ok(Self {
            builder,
            walker,
            with_atime: opts.with_atime,
            ignore_devid: opts.ignore_devid,
            cache: UsersCache::new(),
        })
    }
}

impl ReadSource for LocalSource {
    type Reader = File;
    fn read(path: &Path) -> Result<Self::Reader> {
        Ok(File::open(path)?)
    }
    fn size(&self) -> Result<u64> {
        let mut size = 0;
        for entry in self.builder.build() {
            if let Err(e) = entry.and_then(|e| e.metadata()).map(|m| {
                size += if m.is_dir() { 0 } else { m.len() };
            }) {
                warn!("ignoring error {}", e);
            }
        }
        Ok(size)
    }
}

impl Iterator for LocalSource {
    type Item = Result<(PathBuf, Node)>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.walker.next() {
            // ignore root dir, i.e. an entry with depth 0 of type dir
            Some(Ok(entry)) if entry.depth() == 0 && entry.file_type().unwrap().is_dir() => {
                self.walker.next()
            }
            item => item,
        }
        .map(|e| map_entry(e?, self.with_atime, self.ignore_devid, &self.cache))
    }
}

// map_entry: turn entry into (Path, Node)
fn map_entry(
    entry: DirEntry,
    with_atime: bool,
    ignore_devid: bool,
    cache: &UsersCache,
) -> Result<(PathBuf, Node)> {
    let name = entry.file_name();
    let m = entry.metadata()?;

    let uid = m.uid();
    let gid = m.gid();
    let user = cache
        .get_user_by_uid(uid)
        .map(|u| u.name().to_str().unwrap().to_string());
    let group = cache
        .get_group_by_gid(gid)
        .map(|g| g.name().to_str().unwrap().to_string());

    let mtime = Utc
        .timestamp_opt(m.mtime(), m.mtime_nsec().try_into()?)
        .single()
        .map(|dt| dt.with_timezone(&Local));
    let atime = if with_atime {
        Utc.timestamp_opt(m.atime(), m.atime_nsec().try_into()?)
            .single()
            .map(|dt| dt.with_timezone(&Local))
    } else {
        // TODO: Use None here?
        mtime
    };
    let ctime = Utc
        .timestamp_opt(m.ctime(), m.ctime_nsec().try_into()?)
        .single()
        .map(|dt| dt.with_timezone(&Local));
    let size = if m.is_dir() { 0 } else { m.len() };
    let mode = map_mode_to_go(m.mode());
    let inode = m.ino();
    let device_id = if ignore_devid { 0 } else { m.dev() };
    let links = if m.is_dir() { 0 } else { m.nlink() };

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
    };
    let filetype = m.file_type();

    let node = if m.is_dir() {
        Node::new_node(name, NodeType::Dir, meta)
    } else if m.is_symlink() {
        let target = read_link(entry.path())?;
        let node_type = NodeType::Symlink {
            linktarget: target.to_str().expect("no unicode").to_string(),
        };
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
    Ok((entry.path().to_path_buf(), node))
}

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

/// map st_mode from POSIX (inode(7)) to golang's definition (https://pkg.go.dev/io/fs#ModeType)
/// Note, that it only sets the bits os.ModePerm | os.ModeType | os.ModeSetuid | os.ModeSetgid | os.ModeSticky
/// to stay compatible with the restic implementation
fn map_mode_to_go(mode: u32) -> u32 {
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

/// map gloangs mode definition (https://pkg.go.dev/io/fs#ModeType) to t_mode from POSIX (inode(7))
/// This is the inverse function to map_mode_to_go()
pub fn map_mode_from_go(go_mode: u32) -> u32 {
    let mut mode = go_mode & MODE_PERM;

    if go_mode & GO_MODE_SOCKET > 0 {
        mode |= S_IFSOCK
    } else if go_mode & GO_MODE_SYMLINK > 0 {
        mode |= S_IFLNK
    } else if go_mode & GO_MODE_DEVICE > 0 && go_mode & GO_MODE_CHARDEV == 0 {
        mode |= S_IFBLK;
    } else if go_mode & GO_MODE_DIR > 0 {
        mode |= S_IFDIR;
    } else if go_mode & (GO_MODE_CHARDEV | GO_MODE_DEVICE) > 0 {
        mode |= S_IFCHR;
    } else if go_mode & GO_MODE_FIFO > 0 {
        mode |= S_IFIFO;
    } else if go_mode & GO_MODE_IRREG > 0 {
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
