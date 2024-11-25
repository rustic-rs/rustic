#[cfg(not(windows))]
use std::os::unix::prelude::OsStrExt;
use std::{
    collections::BTreeMap,
    ffi::{CString, OsStr},
    path::Path,
    sync::RwLock,
    time::{Duration, SystemTime},
};

use rustic_core::{
    repofile::{Node, NodeType},
    vfs::{FilePolicy, OpenFile, Vfs},
    IndexedFull, Repository,
};

use fuse_mt::{
    CallbackResult, DirectoryEntry, FileAttr, FileType, FilesystemMT, RequestInfo, ResultData,
    ResultEmpty, ResultEntry, ResultOpen, ResultReaddir, ResultSlice, ResultXattr, Xattr,
};
use itertools::Itertools;

pub struct FuseFS<P, S> {
    repo: Repository<P, S>,
    vfs: Vfs,
    open_files: RwLock<BTreeMap<u64, OpenFile>>,
    now: SystemTime,
    file_policy: FilePolicy,
}

impl<P, S: IndexedFull> FuseFS<P, S> {
    pub(crate) fn new(repo: Repository<P, S>, vfs: Vfs, file_policy: FilePolicy) -> Self {
        let open_files = RwLock::new(BTreeMap::new());

        Self {
            repo,
            vfs,
            open_files,
            now: SystemTime::now(),
            file_policy,
        }
    }

    fn node_from_path(&self, path: &Path) -> Result<Node, i32> {
        self.vfs
            .node_from_path(&self.repo, path)
            .map_err(|_| libc::ENOENT)
    }

    fn dir_entries_from_path(&self, path: &Path) -> Result<Vec<Node>, i32> {
        self.vfs
            .dir_entries_from_path(&self.repo, path)
            .map_err(|_| libc::ENOENT)
    }
}

fn node_to_filetype(node: &Node) -> FileType {
    match node.node_type {
        NodeType::File => FileType::RegularFile,
        NodeType::Dir => FileType::Directory,
        NodeType::Symlink { .. } => FileType::Symlink,
        NodeType::Chardev { .. } => FileType::CharDevice,
        NodeType::Dev { .. } => FileType::BlockDevice,
        NodeType::Fifo => FileType::NamedPipe,
        NodeType::Socket => FileType::Socket,
    }
}

fn node_type_to_rdev(tpe: &NodeType) -> u32 {
    u32::try_from(match tpe {
        NodeType::Dev { device } | NodeType::Chardev { device } => *device,
        _ => 0,
    })
    .unwrap()
}

fn node_to_linktarget(node: &Node) -> Option<&OsStr> {
    if node.is_symlink() {
        Some(node.node_type.to_link().as_os_str())
    } else {
        None
    }
}

fn node_to_file_attr(node: &Node, now: SystemTime) -> FileAttr {
    FileAttr {
        // Size in bytes
        size: node.meta.size,
        // Size in blocks
        blocks: 0,
        // Time of last access
        atime: node.meta.atime.map(SystemTime::from).unwrap_or(now),
        // Time of last modification
        mtime: node.meta.mtime.map(SystemTime::from).unwrap_or(now),
        // Time of last metadata change
        ctime: node.meta.ctime.map(SystemTime::from).unwrap_or(now),
        // Time of creation (macOS only)
        crtime: now,
        // Kind of file (directory, file, pipe, etc.)
        kind: node_to_filetype(node),
        // Permissions
        perm: node.meta.mode.unwrap_or(0o755) as u16,
        // Number of hard links
        nlink: node.meta.links.try_into().unwrap_or(1),
        // User ID
        uid: node.meta.uid.unwrap_or(0),
        // Group ID
        gid: node.meta.gid.unwrap_or(0),
        // Device ID (if special file)
        rdev: node_type_to_rdev(&node.node_type),
        // Flags (macOS only; see chflags(2))
        flags: 0,
    }
}

impl<P, S: IndexedFull> FilesystemMT for FuseFS<P, S> {
    fn getattr(&self, _req: RequestInfo, path: &Path, _fh: Option<u64>) -> ResultEntry {
        let node = self.node_from_path(path)?;
        Ok((Duration::from_secs(1), node_to_file_attr(&node, self.now)))
    }

    #[cfg(not(windows))]
    fn readlink(&self, _req: RequestInfo, path: &Path) -> ResultData {
        let target = node_to_linktarget(&self.node_from_path(path)?)
            .ok_or(libc::ENOSYS)?
            .as_bytes()
            .to_vec();

        Ok(target)
    }

    fn open(&self, _req: RequestInfo, path: &Path, _flags: u32) -> ResultOpen {
        if matches!(self.file_policy, FilePolicy::Forbidden) {
            return Err(libc::ENOTSUP);
        }
        let node = self.node_from_path(path)?;
        let open = self.repo.open_file(&node).map_err(|_| libc::ENOSYS)?;
        let fh = {
            let mut open_files = self.open_files.write().unwrap();
            let fh = open_files.last_key_value().map_or(0, |(fh, _)| *fh + 1);
            _ = open_files.insert(fh, open);
            fh
        };
        Ok((fh, 0))
    }

    fn release(
        &self,
        _req: RequestInfo,
        _path: &Path,
        fh: u64,
        _flags: u32,
        _lock_owner: u64,
        _flush: bool,
    ) -> ResultEmpty {
        _ = self.open_files.write().unwrap().remove(&fh);
        Ok(())
    }

    fn read(
        &self,
        _req: RequestInfo,
        _path: &Path,
        fh: u64,
        offset: u64,
        size: u32,
        callback: impl FnOnce(ResultSlice<'_>) -> CallbackResult,
    ) -> CallbackResult {
        if let Some(open_file) = self.open_files.read().unwrap().get(&fh) {
            if let Ok(data) =
                self.repo
                    .read_file_at(open_file, offset.try_into().unwrap(), size as usize)
            {
                return callback(Ok(&data));
            }
        }
        callback(Err(libc::ENOSYS))
    }

    fn opendir(&self, _req: RequestInfo, _path: &Path, _flags: u32) -> ResultOpen {
        Ok((0, 0))
    }

    fn readdir(&self, _req: RequestInfo, path: &Path, _fh: u64) -> ResultReaddir {
        let nodes = self.dir_entries_from_path(path)?;

        let result = nodes
            .into_iter()
            .map(|node| DirectoryEntry {
                name: node.name(),
                kind: node_to_filetype(&node),
            })
            .collect();
        Ok(result)
    }

    fn releasedir(&self, _req: RequestInfo, _path: &Path, _fh: u64, _flags: u32) -> ResultEmpty {
        Ok(())
    }

    fn listxattr(&self, _req: RequestInfo, path: &Path, size: u32) -> ResultXattr {
        let node = self.node_from_path(path)?;
        let xattrs = node
            .meta
            .extended_attributes
            .into_iter()
            // convert into null-terminated [u8]
            .map(|a| CString::new(a.name).unwrap().into_bytes_with_nul())
            .concat();

        if size == 0 {
            Ok(Xattr::Size(u32::try_from(xattrs.len()).unwrap()))
        } else {
            Ok(Xattr::Data(xattrs))
        }
    }

    fn getxattr(&self, _req: RequestInfo, path: &Path, name: &OsStr, size: u32) -> ResultXattr {
        let node = self.node_from_path(path)?;
        match node
            .meta
            .extended_attributes
            .into_iter()
            .find(|a| name == OsStr::new(&a.name))
        {
            None => Err(libc::ENOSYS),
            Some(attr) => {
                let value = attr.value.unwrap_or_default();
                if size == 0 {
                    Ok(Xattr::Size(u32::try_from(value.len()).unwrap()))
                } else {
                    Ok(Xattr::Data(value))
                }
            }
        }
    }
}
