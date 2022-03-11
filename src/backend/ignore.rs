use std::fs::{read_link, File};
use std::os::linux::fs::MetadataExt;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{TimeZone, Utc};
use clap::Parser;
use ignore::{overrides::OverrideBuilder, DirEntry, Walk, WalkBuilder};
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
        let mut walk_builder = WalkBuilder::new(backup_path.clone());
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

    fn next(&mut self) -> std::option::Option<Self::Item> {
        self.walker
            .next()
            .map(|e| map_entry(e?, self.with_atime, &self.cache))
    }
}

// map_entry: turn entry into (Path, Node)
fn map_entry(entry: DirEntry, with_atime: bool, cache: &UsersCache) -> Result<(PathBuf, Node)> {
    let name = entry.file_name().to_os_string();
    let m = entry.metadata()?;

    let uid = m.st_uid();
    let gid = m.st_gid();
    let user = cache
        .get_user_by_uid(uid)
        .map(|u| u.name().to_str().unwrap().to_string());
    let group = cache
        .get_group_by_gid(gid)
        .map(|g| g.name().to_str().unwrap().to_string());

    let mtime = Some(
        Utc.timestamp(m.st_mtime(), m.st_mtime_nsec().try_into()?)
            .into(),
    );
    let atime = if with_atime {
        Some(
            Utc.timestamp(m.st_atime(), m.st_atime_nsec().try_into()?)
                .into(),
        )
    } else {
        // TODO: Use None here?
        mtime
    };
    let ctime = Some(
        Utc.timestamp(m.st_ctime(), m.st_ctime_nsec().try_into()?)
            .into(),
    );
    let size = if m.is_dir() { 0 } else { m.len() };
    let mode = m.st_mode();
    let inode = m.st_ino();
    let device_id = m.st_dev();
    let links = m.st_nlink();

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
