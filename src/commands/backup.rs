use std::ffi::OsString;
use std::fs::{read_link, File};
use std::io::{BufRead, BufReader};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use chrono::{TimeZone, Utc};
use clap::Parser;
use gethostname::gethostname;
use ignore::{overrides::OverrideBuilder, DirEntry, WalkBuilder};
use path_absolutize::*;
use users::{cache::UsersCache, Groups, Users};
use vlog::*;

use crate::archiver::{Archiver, Parent};
use crate::backend::{DecryptFullBackend, DryRunBackend};
use crate::blob::{Metadata, Node};
use crate::index::IndexBackend;
use crate::repo::{ConfigFile, SnapshotFile};
#[derive(Parser)]
pub(super) struct Opts {
    /// Do not upload or write any data, just show what would be done
    #[clap(long, short = 'n')]
    dry_run: bool,

    /// Save access time for files and directories
    #[clap(long)]
    with_atime: bool,

    /// Snapshot to use as parent
    #[clap(long, value_name = "SNAPSHOT", conflicts_with = "force")]
    parent: Option<String>,

    /// Use no parent, read all files
    #[clap(long, short, conflicts_with = "parent")]
    force: bool,

    /// Exclude other file systems, don't cross filesystem boundaries and subvolumes
    #[clap(long, short = 'x')]
    one_file_system: bool,

    /// glob pattern to include/exclue (can be specified multiple times)
    #[clap(long, short = 'e')]
    glob: Vec<String>,

    /// Read glob patterns to exclude/include from a file (can be specified multiple times)
    #[clap(long, value_name = "FILE")]
    glob_file: Vec<String>,

    /// Exclude contents of directories containing filename (can be specified multiple times)
    #[clap(long, value_name = "FILE")]
    exclude_if_present: Vec<String>,

    /// Same as --glob pattern but ignores the casing of filenames
    #[clap(long, value_name = "GLOB")]
    iglob: Vec<String>,

    /// Same as --glob-file ignores the casing of filenames in patterns
    #[clap(long, value_name = "FILE")]
    iglob_file: Vec<String>,

    /// backup source
    source: String,
}

pub(super) fn execute(opts: Opts, be: &impl DecryptFullBackend) -> Result<()> {
    let config = ConfigFile::from_backend_no_id(be)?;

    let be = DryRunBackend::new(be.clone(), opts.dry_run);

    let poly = u64::from_str_radix(config.chunker_polynomial(), 16)?;
    let backup_path = PathBuf::from(&opts.source);
    let backup_path = backup_path.absolutize()?;
    let backup_path_str = backup_path
        .to_str()
        .ok_or_else(|| anyhow!("non-unicode path {:?}", backup_path))?
        .to_string();

    let hostname = gethostname();
    let parent = match (opts.force, opts.parent) {
        (true, _) => None,
        (false, None) => SnapshotFile::latest(&be, |snap| {
            OsString::from(&snap.hostname) == hostname && snap.paths.contains(&backup_path_str)
        })
        .ok(),
        (false, Some(parent)) => SnapshotFile::from_id(&be, &parent).ok(),
    };
    let parent_tree = match parent {
        Some(snap) => {
            v1!("using parent {}", snap.id);
            Some(snap.tree)
        }
        None => {
            v1!("using no parent");
            None
        }
    };

    let index = IndexBackend::only_full_trees(&be)?;

    let parent = Parent::new(&index, parent_tree.as_ref());
    let mut archiver = Archiver::new(be, index, poly, parent)?;

    let mut walk_builder = WalkBuilder::new(backup_path.clone());
    /*
     for path in &paths[1..] {
        wb.add(path);
    }
    */

    let mut override_builder = OverrideBuilder::new("/");

    // TODO: Add opts.glob-file // opts.iglob-file
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
        .git_ignore(false)
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

    let cache = UsersCache::new();

    let nodes = walk_builder
        .build()
        .map(|entry| map_entry(entry?, opts.with_atime, &cache));

    for res in nodes {
        let (path, node, r) = res?;
        archiver.add_entry(&path, node, r)?;
    }

    let mut snap = SnapshotFile::default();
    snap.set_paths(vec![backup_path.to_path_buf()]);
    snap.set_hostname(hostname);
    archiver.finalize_snapshot(snap)?;

    Ok(())
}

fn map_entry(
    entry: DirEntry,
    with_atime: bool,
    cache: &UsersCache,
) -> Result<(PathBuf, Node, Option<impl BufRead>)> {
    let name = entry.file_name().to_os_string();
    let m = entry.metadata()?;

    let uid = m.uid();
    let gid = m.gid();

    let meta = Metadata {
        size: m.len(),
        mtime: m.modified().ok().map(|t| t.into()),
        atime: if with_atime {
            m.accessed().ok().map(|t| t.into())
        } else {
            // TODO: Use None here?
            m.modified().ok().map(|t| t.into())
        },
        ctime: Some(Utc.timestamp(m.ctime(), m.ctime_nsec().try_into()?).into()),
        mode: m.mode(),
        uid,
        gid,
        user: cache
            .get_user_by_uid(uid)
            .map(|u| u.name().to_str().unwrap().to_string()),
        group: cache
            .get_group_by_gid(gid)
            .map(|g| g.name().to_str().unwrap().to_string()),
        inode: m.ino(),
        device_id: m.dev(),
        links: m.nlink(),
    };
    let (node, r) = if m.is_dir() {
        (Node::new_dir(name, meta), None)
    } else if m.is_symlink() {
        let target = read_link(entry.path())?;
        (Node::new_symlink(name, target, meta), None)
    } else {
        // TODO: lazily open file! - might be contained in parent
        let f = File::open(&entry.path())?;
        (Node::new_file(name, meta), Some(BufReader::new(f)))
    };
    Ok((entry.path().to_path_buf(), node, r))
}
