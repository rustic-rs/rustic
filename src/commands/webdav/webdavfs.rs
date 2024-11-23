#[cfg(not(windows))]
use std::os::unix::ffi::OsStrExt;
use std::{
    fmt::{Debug, Formatter},
    io::SeekFrom,
    sync::OnceLock,
    time::SystemTime,
};

use bytes::{Buf, Bytes};
use dav_server::{
    davpath::DavPath,
    fs::{
        DavDirEntry, DavFile, DavFileSystem, DavMetaData, FsError, FsFuture, FsResult, FsStream,
        OpenOptions, ReadDirMeta,
    },
};
use futures::FutureExt;
use tokio::sync::{mpsc, oneshot};

use rustic_core::{
    repofile::Node,
    vfs::{FilePolicy, OpenFile, Vfs},
    IndexedFull, Repository,
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

impl<P, S: IndexedFull> DavFsInner<P, S> {
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
        self.vfs
            .node_from_path(&self.repo, &path.as_pathbuf())
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
        self.vfs
            .dir_entries_from_path(&self.repo, &path.as_pathbuf())
            .map_err(|_| FsError::GeneralFailure)
    }

    fn open(&self, node: &Node, options: OpenOptions) -> Result<OpenFile, FsError> {
        if options.write
            || options.append
            || options.truncate
            || options.create
            || options.create_new
        {
            return Err(FsError::Forbidden);
        }

        if matches!(self.file_policy, FilePolicy::Forbidden) {
            return Err(FsError::Forbidden);
        }

        let open = self
            .repo
            .open_file(node)
            .map_err(|_err| FsError::GeneralFailure)?;
        Ok(open)
    }

    fn read_bytes(
        &self,
        file: OpenFile,
        seek: usize,
        count: usize,
    ) -> Result<(Bytes, OpenFile), FsError> {
        let data = self
            .repo
            .read_file_at(&file, seek, count)
            .map_err(|_err| FsError::GeneralFailure)?;
        Ok((data, file))
    }
}

/// Messages used
#[allow(clippy::large_enum_variant)]
enum DavFsInnerCommand {
    Node(DavPath, oneshot::Sender<Result<Node, FsError>>),
    DirEntries(DavPath, oneshot::Sender<Result<Vec<Node>, FsError>>),
    Open(
        Node,
        OpenOptions,
        oneshot::Sender<Result<OpenFile, FsError>>,
    ),
    ReadBytes(
        OpenFile,
        usize,
        usize,
        oneshot::Sender<Result<(Bytes, OpenFile), FsError>>,
    ),
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
#[derive(Debug, Clone)]
pub struct WebDavFS {
    send: mpsc::Sender<DavFsInnerCommand>,
}

impl WebDavFS {
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
    pub(crate) fn new<P: Send + 'static, S: IndexedFull + Send + 'static>(
        repo: Repository<P, S>,
        vfs: Vfs,
        file_policy: FilePolicy,
    ) -> Self {
        let inner = DavFsInner {
            repo,
            vfs,
            file_policy,
        };

        let (send, mut rcv) = mpsc::channel(1);

        let _ = std::thread::spawn(move || -> Result<_, FsError> {
            while let Some(task) = rcv.blocking_recv() {
                match task {
                    DavFsInnerCommand::Node(path, res) => {
                        res.send(inner.node_from_path(&path))
                            .map_err(|_err| FsError::GeneralFailure)?;
                    }
                    DavFsInnerCommand::DirEntries(path, res) => {
                        res.send(inner.dir_entries_from_path(&path))
                            .map_err(|_err| FsError::GeneralFailure)?;
                    }
                    DavFsInnerCommand::Open(path, open_options, res) => {
                        res.send(inner.open(&path, open_options))
                            .map_err(|_err| FsError::GeneralFailure)?;
                    }
                    DavFsInnerCommand::ReadBytes(file, seek, count, res) => {
                        res.send(inner.read_bytes(file, seek, count))
                            .map_err(|_err| FsError::GeneralFailure)?;
                    }
                }
            }
            Ok(())
        });

        Self { send }
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
    async fn node_from_path(&self, path: &DavPath) -> Result<Node, FsError> {
        let (send, rcv) = oneshot::channel();
        self.send
            .send(DavFsInnerCommand::Node(path.clone(), send))
            .await
            .map_err(|_err| FsError::GeneralFailure)?;
        rcv.await.map_err(|_err| FsError::GeneralFailure)?
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
    async fn dir_entries_from_path(&self, path: &DavPath) -> Result<Vec<Node>, FsError> {
        let (send, rcv) = oneshot::channel();
        self.send
            .send(DavFsInnerCommand::DirEntries(path.clone(), send))
            .await
            .map_err(|_err| FsError::GeneralFailure)?;
        rcv.await.map_err(|_err| FsError::GeneralFailure)?
    }

    async fn open(&self, node: &Node, options: OpenOptions) -> Result<OpenFile, FsError> {
        let (send, rcv) = oneshot::channel();
        self.send
            .send(DavFsInnerCommand::Open(node.clone(), options, send))
            .await
            .map_err(|_err| FsError::GeneralFailure)?;
        rcv.await.map_err(|_err| FsError::GeneralFailure)?
    }
    async fn read_bytes(
        &self,
        file: OpenFile,
        seek: usize,
        count: usize,
    ) -> Result<(Bytes, OpenFile), FsError> {
        let (send, rcv) = oneshot::channel();
        self.send
            .send(DavFsInnerCommand::ReadBytes(file, seek, count, send))
            .await
            .map_err(|_err| FsError::GeneralFailure)?;
        rcv.await.map_err(|_err| FsError::GeneralFailure)?
    }
}

impl DavFileSystem for WebDavFS {
    fn metadata<'a>(&'a self, davpath: &'a DavPath) -> FsFuture<'_, Box<dyn DavMetaData>> {
        self.symlink_metadata(davpath)
    }

    fn symlink_metadata<'a>(&'a self, davpath: &'a DavPath) -> FsFuture<'_, Box<dyn DavMetaData>> {
        async move {
            let node = self.node_from_path(davpath).await?;
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
            let entries = self.dir_entries_from_path(davpath).await?;
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
            let node = self.node_from_path(path).await?;
            let file = self.open(&node, options).await?;
            let file: Box<dyn DavFile> = Box::new(DavFsFile {
                open: Some(file),
                seek: 0,
                fs: self.clone(),
                node,
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
struct DavFsFile {
    /// The [`Node`] this file is for
    node: Node,

    /// The [`OpenFile`] for this file
    open: Option<OpenFile>,

    /// The current seek position
    seek: usize,
    fs: WebDavFS,
}

impl Debug for DavFsFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "DavFile")
    }
}

impl DavFile for DavFsFile {
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
            let (data, open) = self
                .fs
                .read_bytes(
                    self.open.take().ok_or(FsError::GeneralFailure)?,
                    self.seek,
                    count,
                )
                .await?;
            self.seek += data.len();
            self.open = Some(open);
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
