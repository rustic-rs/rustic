use crate::backend::{map_mode_to_go, ReadSource, ReadSourceEntry, ReadSourceOpen};
use crate::blob::{Metadata, Node, NodeType};
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Local, TimeZone, Utc};
use ouroboros::self_referencing;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::{io, mem};
use tar::EntryType;

pub struct TarSource<R: Read + 'static>(Arc<Data<R>>);

pub struct TarFile<R: Read + 'static>(Arc<Data<R>>);

pub struct TarOpen<R: Read + 'static>(TarFile<R>);

pub struct TarEntries<R: Read + 'static>(Arc<Data<R>>);

impl<R: Read + 'static> TarSource<R> {
    pub fn new(archive: tar::Archive<R>) -> io::Result<Self> {
        let result = ReaderTryBuilder {
            archive,
            entries_builder: |archive| {
                archive
                    .entries()
                    .map(|entries| Entries::NoCurrentFile { entries })
            },
        }
        .try_build()?;
        Ok(Self(Arc::new(Data::new(result))))
    }
}

impl<R: Read + Send + 'static> ReadSource for TarSource<R> {
    type Open = TarOpen<R>;
    type Iter = TarEntries<R>;

    fn size(&self) -> Result<Option<u64>> {
        Ok(None)
    }

    fn entries(self) -> Self::Iter {
        TarEntries(self.0)
    }
}

impl<R: Read + 'static> Iterator for TarEntries<R> {
    type Item = Result<ReadSourceEntry<TarOpen<R>>>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut reader = self.0.data.lock().unwrap();
        loop {
            let res = match reader.with_entries_mut(|entries| entries.read_entry()) {
                EntryReadResult::NoMoreEntries => None,
                EntryReadResult::Error(e) => Some(Err(e)),
                EntryReadResult::StartedReader(mut entry) => {
                    entry.open = Some(TarOpen(TarFile(Arc::clone(&self.0))));
                    Some(Ok(*entry))
                }
                EntryReadResult::WaitForReaderDone => {
                    reader = self.0.entry_read.wait(reader).unwrap();
                    continue;
                }
            };
            return res;
        }
    }
}

fn timestamp_from_maybe_secs<E: std::error::Error + Sync + Send + 'static>(
    s: Result<u64, E>,
) -> Result<DateTime<Local>> {
    let n = s?;
    let n = i64::try_from(n).context("Tar timestamp out of range for i64")?;
    let dt = Utc
        .timestamp_opt(n, 0)
        .single()
        .with_context(|| format!("Cannot create timestamp from tar timestamp: {n}"))?;
    Ok(dt.with_timezone(&Local))
}

fn make_dev(header: &tar::Header) -> Result<u64> {
    let major = header.device_major()?.context("no major device")?;
    let minor = header.device_minor()?.context("no minor device")?;

    Ok(u64::from(major << 24 | minor))
}

fn path_node_from_entry<R: Read>(entry: &tar::Entry<R>) -> Result<(PathBuf, Node)> {
    let path = entry.path()?;
    let name = path.file_name().context("expected a file name")?;
    let header = entry.header();
    let size = entry.size();
    let mtime = timestamp_from_maybe_secs(header.mtime())?;

    let uid = header.uid()?.try_into().context("uid too large")?;
    let gid = header.gid()?.try_into().context("gid too large")?;
    let user = header.username()?.map(String::from);
    let group = header.groupname()?.map(String::from);
    let mode = map_mode_to_go(header.mode()?);

    let meta = Metadata {
        size,
        mtime: Some(mtime),
        atime: None,
        ctime: Some(mtime),
        mode: Some(mode),
        uid: Some(uid),
        gid: Some(gid),
        user,
        group,
        inode: 0,
        device_id: 0,
        links: 0,
    };
    let node_type = match header.entry_type() {
        EntryType::Regular | EntryType::GNUSparse | EntryType::Continuous => NodeType::File,
        EntryType::Symlink => {
            let name_bytes = header
                .link_name_bytes()
                .context("Expected symlink entry to contain link name")?;
            let link_name = String::from_utf8(name_bytes.into_owned())
                .context("symlink name must be unicode")?;
            NodeType::Symlink {
                linktarget: link_name,
            }
        }
        EntryType::Char => NodeType::Chardev {
            device: make_dev(header)?,
        },
        EntryType::Block => NodeType::Dev {
            device: make_dev(header)?,
        },
        EntryType::Directory => NodeType::Dir,
        EntryType::Fifo => NodeType::Fifo,

        EntryType::Link | EntryType::GNULongLink => bail!("hard links not implemented"),
        other => bail!("unimplemented entry type: {other:?}"),
    };
    let node = Node::new_node(name, node_type, meta);
    Ok((path.into_owned(), node))
}

#[derive(Default)]
enum Entries<'a, R: Read> {
    NoCurrentFile {
        entries: tar::Entries<'a, R>,
    },
    Reading {
        entries: tar::Entries<'a, R>,
        entry: Box<tar::Entry<'a, R>>,
    },
    // Used as an in-between state, to allow stealing the
    #[default]
    Empty,
}

enum EntryReadResult<R: Read + 'static> {
    StartedReader(Box<ReadSourceEntry<TarOpen<R>>>),
    WaitForReaderDone,
    NoMoreEntries,
    Error(anyhow::Error),
}

impl<'a, R: Read> Entries<'a, R> {
    fn done_with_entry(&mut self) {
        match mem::take(self) {
            Entries::Reading { entries, .. } => *self = Self::NoCurrentFile { entries },
            _ => panic!("expected to be reading a tar entry"),
        }
    }

    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Entries::Reading { entry, .. } => entry.read(buf),
            _ => panic!("expected to be reading a tar entry"),
        }
    }

    fn read_entry(&mut self) -> EntryReadResult<R> {
        let (new_self, result) = match mem::take(self) {
            new_self @ Entries::Reading { .. } => (new_self, EntryReadResult::WaitForReaderDone),
            Entries::NoCurrentFile { mut entries } => match entries.next() {
                Some(Ok(entry)) => {
                    match path_node_from_entry(&entry) {
                        Ok((path, node)) => {
                            let new_self = Entries::Reading {
                                entries,
                                entry: Box::new(entry),
                            };
                            (
                                new_self,
                                EntryReadResult::StartedReader(Box::new(ReadSourceEntry {
                                    path,
                                    node,
                                    open: None,
                                })),
                            )
                        }
                        Err(e) => {
                            (Entries::NoCurrentFile { entries }, EntryReadResult::Error(e))
                        }
                    }
                }
                Some(Err(e)) => (
                    Entries::NoCurrentFile { entries },
                    EntryReadResult::Error(e.into()),
                ),
                None => (
                    Entries::NoCurrentFile { entries },
                    EntryReadResult::NoMoreEntries,
                ),
            },
            Entries::Empty => panic!("Never expect to see empty result"),
        };
        *self = new_self;
        result
    }
}

#[self_referencing]
struct Reader<R: Read + 'static> {
    archive: tar::Archive<R>,
    #[borrows(mut archive)]
    #[not_covariant]
    entries: Entries<'this, R>,
}

// SAFETY:
// Reader does not implement Send automatically because it contains (through `entries`) a reference
// to a cell (also indirectly). &Cell is not Send because Cell is not Sync.
// However: The entry's reference points into `archive` (and is not otherwise shared), and we will never separate them,
// all accesses will go through `Reader`, and Reader will only be accessed through a mutex: all writes through the cell
// will be synchronized via the mutex, and cannot be observed without the mutex.
#[allow(unsafe_code)]
unsafe impl<R: Read + 'static> Send for Reader<R> {}

struct Data<R: Read + 'static> {
    data: Mutex<Reader<R>>,
    entry_read: Condvar,
}

impl<R: Read + 'static> Data<R> {
    fn new(reader: Reader<R>) -> Self {
        Data {
            data: Mutex::new(reader),
            entry_read: Condvar::new(),
        }
    }
}

impl<R: Read + 'static> Read for TarFile<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut data = self.0.data.lock().unwrap();
        data.with_entries_mut(|entries| entries.read(buf))
    }
}

impl<R: Read + 'static> Drop for TarFile<R> {
    fn drop(&mut self) {
        let Ok(mut reader) = self.0.data.lock() else { return };
        reader.with_entries_mut(|entries| {
            entries.done_with_entry();
        });
        self.0.entry_read.notify_all();
    }
}

impl<R: Read + Send + 'static> ReadSourceOpen for TarOpen<R> {
    type Reader = TarFile<R>;

    fn open(self, _path: &Path) -> Result<Self::Reader> {
        Ok(self.0)
    }
}
