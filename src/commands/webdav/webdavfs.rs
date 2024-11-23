use std::fmt::{Debug, Formatter};
use std::io::SeekFrom;
#[cfg(not(windows))]
use std::os::unix::ffi::OsStrExt;
use std::sync::{Arc, OnceLock};
use std::time::SystemTime;

use bytes::{Buf, Bytes};
use futures::FutureExt;
use rustic_core::repofile::Node;
use rustic_core::vfs::{FilePolicy, OpenFile, Vfs};
use rustic_core::{IndexedFull, Repository};

use dav_server::{
    davpath::DavPath,
    fs::{
        DavDirEntry, DavFile, DavFileSystem, DavMetaData, FsError, FsFuture, FsResult, FsStream,
        OpenOptions, ReadDirMeta,
    },
};

fn now() -> SystemTime {
    static NOW: OnceLock<SystemTime> = OnceLock::new();
    *NOW.get_or_init(SystemTime::now)
}

/// The inner state of a [`WebDavFS`] instance.
struct DavFsInner<P, S> {
    /// The [`Repository`] to use
    repo: Repository<P, S>,

    /// The [`Vfs`] to use
    vfs: Vfs,

    /// The [`FilePolicy`] to use
    file_policy: FilePolicy,
}

impl<P, S> Debug for DavFsInner<P, S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "DavFS")
    }
}

/// DAV Filesystem implementation.
///
/// This is the main entry point for the DAV filesystem.
/// It implements [`DavFileSystem`] and can be used to serve a [`Repository`] via DAV.
#[derive(Debug)]
pub struct WebDavFS<P, S> {
    inner: Arc<DavFsInner<P, S>>,
}

impl<P, S: IndexedFull> WebDavFS<P, S> {
    /// Create a new [`WebDavFS`] instance.
    ///
    /// # Arguments
    ///
    /// * `repo` - The [`Repository`] to use
    /// * `vfs` - The [`Vfs`] to use
    /// * `file_policy` - The [`FilePolicy`] to use
    ///
    /// # Returns
    ///
    /// A new [`WebDavFS`] instance
    pub(crate) fn new(repo: Repository<P, S>, vfs: Vfs, file_policy: FilePolicy) -> Self {
        let inner = DavFsInner {
            repo,
            vfs,
            file_policy,
        };

        Self {
            inner: Arc::new(inner),
        }
    }

    /// Get a [`Node`] from the specified [`DavPath`].
    ///
    /// # Arguments
    ///
    /// * `path` - The path to get the [`Tree`] at
    ///
    /// # Errors
    ///
    /// * If the [`Tree`] could not be found
    ///
    /// # Returns
    ///
    /// The [`Node`] at the specified path
    ///
    /// [`Tree`]: crate::repofile::Tree
    fn node_from_path(&self, path: &DavPath) -> Result<Node, FsError> {
        self.inner
            .vfs
            .node_from_path(&self.inner.repo, &path.as_pathbuf())
            .map_err(|_| FsError::GeneralFailure)
    }

    /// Get a list of [`Node`]s from the specified directory path.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to get the [`Tree`] at
    ///
    /// # Errors
    ///
    /// * If the [`Tree`] could not be found
    ///
    /// # Returns
    ///
    /// The list of [`Node`]s at the specified path
    ///
    /// [`Tree`]: crate::repofile::Tree
    fn dir_entries_from_path(&self, path: &DavPath) -> Result<Vec<Node>, FsError> {
        self.inner
            .vfs
            .dir_entries_from_path(&self.inner.repo, &path.as_pathbuf())
            .map_err(|_| FsError::GeneralFailure)
    }
}

impl<P, S: IndexedFull> Clone for WebDavFS<P, S> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<P: Debug + Send + Sync + 'static, S: IndexedFull + Debug + Send + Sync + 'static> DavFileSystem
    for WebDavFS<P, S>
{
    fn metadata<'a>(&'a self, davpath: &'a DavPath) -> FsFuture<'_, Box<dyn DavMetaData>> {
        self.symlink_metadata(davpath)
    }

    fn symlink_metadata<'a>(&'a self, davpath: &'a DavPath) -> FsFuture<'_, Box<dyn DavMetaData>> {
        async move {
            let node = self.node_from_path(davpath)?;
            let meta: Box<dyn DavMetaData> = Box::new(DavFsMetaData(node));
            Ok(meta)
        }
        .boxed()
    }

    fn read_dir<'a>(
        &'a self,
        davpath: &'a DavPath,
        _meta: ReadDirMeta,
    ) -> FsFuture<'_, FsStream<Box<dyn DavDirEntry>>> {
        async move {
            let entries = self.dir_entries_from_path(davpath)?;
            let entry_iter = entries.into_iter().map(|e| {
                let entry: Box<dyn DavDirEntry> = Box::new(DavFsDirEntry(e));
                Ok(entry)
            });
            let strm: FsStream<Box<dyn DavDirEntry>> = Box::pin(futures::stream::iter(entry_iter));
            Ok(strm)
        }
        .boxed()
    }

    fn open<'a>(
        &'a self,
        path: &'a DavPath,
        options: OpenOptions,
    ) -> FsFuture<'_, Box<dyn DavFile>> {
        async move {
            if options.write
                || options.append
                || options.truncate
                || options.create
                || options.create_new
            {
                return Err(FsError::Forbidden);
            }

            let node = self.node_from_path(path)?;
            if matches!(self.inner.file_policy, FilePolicy::Forbidden) {
                return Err(FsError::Forbidden);
            }

            let open = self
                .inner
                .repo
                .open_file(&node)
                .map_err(|_err| FsError::GeneralFailure)?;
            let file: Box<dyn DavFile> = Box::new(DavFsFile {
                node,
                open,
                fs: self.inner.clone(),
                seek: 0,
            });
            Ok(file)
        }
        .boxed()
    }
}

/// A [`DavDirEntry`] implementation for [`Node`]s.
#[derive(Clone, Debug)]
struct DavFsDirEntry(Node);

impl DavDirEntry for DavFsDirEntry {
    fn metadata(&self) -> FsFuture<'_, Box<dyn DavMetaData>> {
        async move {
            let meta: Box<dyn DavMetaData> = Box::new(DavFsMetaData(self.0.clone()));
            Ok(meta)
        }
        .boxed()
    }

    #[cfg(not(windows))]
    fn name(&self) -> Vec<u8> {
        self.0.name().as_bytes().to_vec()
    }

    #[cfg(windows)]
    fn name(&self) -> Vec<u8> {
        self.0
            .name()
            .as_os_str()
            .to_string_lossy()
            .to_string()
            .into_bytes()
    }
}

/// A [`DavFile`] implementation for [`Node`]s.
///
/// This is a read-only file.
struct DavFsFile<P, S> {
    /// The [`Node`] this file is for
    node: Node,

    /// The [`OpenFile`] for this file
    open: OpenFile,

    /// The [`DavFsInner`] this file belongs to
    fs: Arc<DavFsInner<P, S>>,

    /// The current seek position
    seek: usize,
}

impl<P, S> Debug for DavFsFile<P, S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "DavFile")
    }
}

impl<P: Debug + Send + Sync, S: IndexedFull + Debug + Send + Sync> DavFile for DavFsFile<P, S> {
    fn metadata(&mut self) -> FsFuture<'_, Box<dyn DavMetaData>> {
        async move {
            let meta: Box<dyn DavMetaData> = Box::new(DavFsMetaData(self.node.clone()));
            Ok(meta)
        }
        .boxed()
    }

    fn write_bytes(&mut self, _buf: Bytes) -> FsFuture<'_, ()> {
        async move { Err(FsError::Forbidden) }.boxed()
    }

    fn write_buf(&mut self, _buf: Box<dyn Buf + Send>) -> FsFuture<'_, ()> {
        async move { Err(FsError::Forbidden) }.boxed()
    }

    fn read_bytes(&mut self, count: usize) -> FsFuture<'_, Bytes> {
        async move {
            let data = self
                .fs
                .repo
                .read_file_at(&self.open, self.seek, count)
                .map_err(|_err| FsError::GeneralFailure)?;
            self.seek += data.len();
            Ok(data)
        }
        .boxed()
    }

    fn seek(&mut self, pos: SeekFrom) -> FsFuture<'_, u64> {
        async move {
            match pos {
                SeekFrom::Start(start) => {
                    self.seek = usize::try_from(start).expect("usize overflow should not happen");
                }
                SeekFrom::Current(delta) => {
                    self.seek = usize::try_from(
                        i64::try_from(self.seek).expect("i64 wrapped around") + delta,
                    )
                    .expect("usize overflow should not happen");
                }
                SeekFrom::End(end) => {
                    self.seek = usize::try_from(
                        i64::try_from(self.node.meta.size).expect("i64 wrapped around") + end,
                    )
                    .expect("usize overflow should not happen");
                }
            }

            Ok(self.seek as u64)
        }
        .boxed()
    }

    fn flush(&mut self) -> FsFuture<'_, ()> {
        async move { Ok(()) }.boxed()
    }
}

/// A [`DavMetaData`] implementation for [`Node`]s.
#[derive(Clone, Debug)]
struct DavFsMetaData(Node);

impl DavMetaData for DavFsMetaData {
    fn len(&self) -> u64 {
        self.0.meta.size
    }
    fn created(&self) -> FsResult<SystemTime> {
        Ok(now())
    }
    fn modified(&self) -> FsResult<SystemTime> {
        Ok(self.0.meta.mtime.map_or_else(now, SystemTime::from))
    }
    fn accessed(&self) -> FsResult<SystemTime> {
        Ok(self.0.meta.atime.map_or_else(now, SystemTime::from))
    }

    fn status_changed(&self) -> FsResult<SystemTime> {
        Ok(self.0.meta.ctime.map_or_else(now, SystemTime::from))
    }

    fn is_dir(&self) -> bool {
        self.0.is_dir()
    }
    fn is_file(&self) -> bool {
        self.0.is_file()
    }
    fn is_symlink(&self) -> bool {
        self.0.is_symlink()
    }
    fn executable(&self) -> FsResult<bool> {
        if self.0.is_file() {
            let Some(mode) = self.0.meta.mode else {
                return Ok(false);
            };
            return Ok((mode & 0o100) > 0);
        }
        Err(FsError::NotImplemented)
    }
}
