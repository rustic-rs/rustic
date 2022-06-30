use std::fs::{read_link, File};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{TimeZone, Utc};
use clap::Parser;
use ignore::{overrides::OverrideBuilder, DirEntry, Walk, WalkBuilder};
#[cfg(not(windows))]
use users::{Groups, Users, UsersCache};

use super::{node::Metadata, Node, ReadSource};

pub struct LocalSource {
    builder: WalkBuilder,
    walker: Walk,
    with_atime: bool,
    cache: UsersCache,
}

#[derive(Parser)]
pub struct LocalSourceOptions {
    /// Save access time for files and directories
    #[clap(long)]
    with_atime: bool,

    /// Exclude other file systems, don't cross filesystem boundaries and subvolumes
    #[clap(long, short = 'x')]
    one_file_system: bool,

    /// Glob pattern to include/exclue (can be specified multiple times)
    #[clap(long, short = 'g')]
    glob: Vec<String>,

    /// Read glob patterns to exclude/include from a file (can be specified multiple times)
    #[clap(long, value_name = "FILE")]
    glob_file: Vec<String>,

    /// Exclude contents of directories containing filename (can be specified multiple times)
    #[clap(long, value_name = "FILE")]
    exclude_if_present: Vec<String>,

    /// Ignore files based on .gitignore files
    #[clap(long)]
    git_ignore: bool,

    /// Same as --glob pattern but ignores the casing of filenames
    #[clap(long, value_name = "GLOB")]
    iglob: Vec<String>,

    /// Same as --glob-file ignores the casing of filenames in patterns
    #[clap(long, value_name = "FILE")]
    iglob_file: Vec<String>,
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

        let with_atime = opts.with_atime;
        let cache = UsersCache::new();
        let builder = walk_builder;
        let walker = builder.build();

        Ok(Self {
            builder,
            walker,
            with_atime,
            cache,
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
                eprintln!("ignoring error {}", e);
            }
        }
        Ok(size)
    }
}

impl Iterator for LocalSource {
    type Item = Result<(PathBuf, Node)>;

    fn next(&mut self) -> Option<Self::Item> {
        self.walker
            .next()
            .map(|e| map_entry(e?, self.with_atime, &self.cache))
    }
}

// map_entry: turn entry into (Path, Node)
fn map_entry(entry: DirEntry, with_atime: bool, cache: &UsersCache) -> Result<(PathBuf, Node)> {
    let name = entry.file_name().to_os_string();
    let m = entry.metadata()?;

    let uid = m.uid();
    let gid = m.gid();
    let user = cache
        .get_user_by_uid(uid)
        .map(|u| u.name().to_str().unwrap().to_string());
    let group = cache
        .get_group_by_gid(gid)
        .map(|g| g.name().to_str().unwrap().to_string());

    let mtime = Some(Utc.timestamp(m.mtime(), m.mtime_nsec().try_into()?).into());
    let atime = if with_atime {
        Some(Utc.timestamp(m.atime(), m.atime_nsec().try_into()?).into())
    } else {
        // TODO: Use None here?
        mtime
    };
    let ctime = Some(Utc.timestamp(m.ctime(), m.ctime_nsec().try_into()?).into());
    let size = if m.is_dir() { 0 } else { m.len() };
    let mode = map_mode_to_go(m.mode());
    let inode = m.ino();
    let device_id = m.dev();
    let links = if m.is_dir() { 0 } else { m.nlink() };

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
    };
    let node = if m.is_dir() {
        Node::new_dir(name, meta)
    } else if m.is_symlink() {
        let target = read_link(entry.path())?;
        Node::new_symlink(name, target, meta)
    } else {
        Node::new_file(name, meta)
    };
    Ok((entry.path().to_path_buf(), node))
}

const MODE_PERM: u32 = 0o777; // permission bits

// consts from https://pkg.go.dev/io/fs#ModeType
const GO_MODE_DIR: u32 = 0b10000000000000000000000000000000;
const GO_MODE_SYMLINK: u32 = 0b00000100000000000000000000000000;
const GO_MODE_DEVICE: u32 = 0b00000010000000000000000000000000;
const GO_MODE_FIFO: u32 = 0b00000001000000000000000000000000;
const GO_MODE_SOCKET: u32 = 0b00000000100000000000000000000000;
const GO_MODE_SETUID: u32 = 0b00000000010000000000000000000000;
const GO_MODE_SETGID: u32 = 0b00000000001000000000000000000000;
const GO_MODE_CHARDEV: u32 = 0b00000000000100000000000000000000;
const GO_MODE_STICKY: u32 = 0b00000000000010000000000000000000;
const GO_MODE_IRREG: u32 = 0b00000000000001000000000000000000;

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
        S_IFCHR => go_mode |= GO_MODE_CHARDEV,
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
