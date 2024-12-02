//! `dump` subcommand

use std::{
    fs::File,
    io::{copy, Cursor, Read, Seek, SeekFrom, Write},
    path::PathBuf,
};

use crate::{repository::CliIndexedRepo, status_err, Application, RUSTIC_APP};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use derive_more::FromStr;
use flate2::{write::GzEncoder, Compression};
use log::warn;
use rustic_core::{
    repofile::{Node, NodeType},
    vfs::OpenFile,
    LsOptions,
};
use tar::{Builder, EntryType, Header};
use zip::{write::SimpleFileOptions, ZipWriter};

/// `dump` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct DumpCmd {
    /// file from snapshot to dump
    #[clap(value_name = "SNAPSHOT[:PATH]")]
    snap: String,

    /// set archive format to use. Possible values: auto, content, tar, targz, zip. For "auto" format is dertermined by file extension (if given) or "tar" for dirs.
    #[clap(long, value_name = "FORMAT", default_value = "auto")]
    archive: ArchiveKind,

    /// dump output to the given file. Use this instead of redirecting stdout to a file.
    #[clap(long)]
    file: Option<PathBuf>,

    /// Glob pattern to exclude/include (can be specified multiple times)
    #[clap(long, help_heading = "Exclude options")]
    glob: Vec<String>,

    /// Same as --glob pattern but ignores the casing of filenames
    #[clap(long, value_name = "GLOB", help_heading = "Exclude options")]
    iglob: Vec<String>,

    /// Read glob patterns to exclude/include from this file (can be specified multiple times)
    #[clap(long, value_name = "FILE", help_heading = "Exclude options")]
    glob_file: Vec<String>,

    /// Same as --glob-file ignores the casing of filenames in patterns
    #[clap(long, value_name = "FILE", help_heading = "Exclude options")]
    iglob_file: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromStr)]
enum ArchiveKind {
    Auto,
    Content,
    Tar,
    TarGz,
    Zip,
}

impl Runnable for DumpCmd {
    fn run(&self) {
        if let Err(err) = RUSTIC_APP
            .config()
            .repository
            .run_indexed(|repo| self.inner_run(repo))
        {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl DumpCmd {
    fn inner_run(&self, repo: CliIndexedRepo) -> Result<()> {
        let config = RUSTIC_APP.config();

        let node =
            repo.node_from_snapshot_path(&self.snap, |sn| config.snapshot_filter.matches(sn))?;

        let stdout = std::io::stdout();

        let ls_opts = LsOptions::default()
            .glob(self.glob.clone())
            .glob_file(self.glob_file.clone())
            .iglob(self.iglob.clone())
            .iglob_file(self.iglob_file.clone())
            .recursive(true);

        let ext = self
            .file
            .as_ref()
            .and_then(|f| f.extension().map(|s| s.to_string_lossy().to_string()));

        let archive = match self.archive {
            ArchiveKind::Auto => match ext.as_deref() {
                Some("tar") => ArchiveKind::Tar,
                Some("tgz") | Some("gz") => ArchiveKind::TarGz,
                Some("zip") => ArchiveKind::Zip,
                _ if node.is_dir() => ArchiveKind::Tar,
                _ => ArchiveKind::Content,
            },
            a => a,
        };

        let mut w: Box<dyn Write> = if let Some(file) = &self.file {
            let mut file = File::create(file)?;
            if archive == ArchiveKind::Zip {
                // when writing zip to a file, we use the optimized writer
                return write_zip_to_file(&repo, &node, &mut file, &ls_opts);
            }
            Box::new(file)
        } else {
            Box::new(stdout)
        };

        match archive {
            ArchiveKind::Content => dump_content(&repo, &node, &mut w, &ls_opts)?,
            ArchiveKind::Tar => dump_tar(&repo, &node, &mut w, &ls_opts)?,
            ArchiveKind::TarGz => dump_tar_gz(&repo, &node, &mut w, &ls_opts)?,
            ArchiveKind::Zip => dump_zip(&repo, &node, &mut w, &ls_opts)?,
            _ => {}
        };

        Ok(())
    }
}

fn dump_content(
    repo: &CliIndexedRepo,
    node: &Node,
    w: &mut impl Write,
    ls_opts: &LsOptions,
) -> Result<()> {
    for item in repo.ls(node, ls_opts)? {
        let (_, node) = item?;
        repo.dump(&node, w)?;
    }
    Ok(())
}

fn dump_tar_gz(
    repo: &CliIndexedRepo,
    node: &Node,
    w: &mut impl Write,
    ls_opts: &LsOptions,
) -> Result<()> {
    let mut w = GzEncoder::new(w, Compression::default());
    dump_tar(repo, node, &mut w, ls_opts)
}

fn dump_tar(
    repo: &CliIndexedRepo,
    node: &Node,
    w: &mut impl Write,
    ls_opts: &LsOptions,
) -> Result<()> {
    let mut ar = Builder::new(w);
    for item in repo.ls(node, ls_opts)? {
        let (path, node) = item?;
        let mut header = Header::new_gnu();

        let entry_type = match &node.node_type {
            NodeType::File => EntryType::Regular,
            NodeType::Dir => EntryType::Directory,
            NodeType::Symlink { .. } => EntryType::Symlink,
            NodeType::Dev { .. } => EntryType::Block,
            NodeType::Chardev { .. } => EntryType::Char,
            NodeType::Fifo => EntryType::Fifo,
            NodeType::Socket => {
                warn!(
                    "socket is not supported. Adding {} as empty file",
                    path.display()
                );
                EntryType::Regular
            }
        };
        header.set_entry_type(entry_type);
        header.set_size(node.meta.size);
        if let Some(mode) = node.meta.mode {
            // TODO: this is some go-mapped mode, but lower bits are the standard unix mode bits -> is this ok?
            header.set_mode(mode);
        }
        if let Some(uid) = node.meta.uid {
            header.set_uid(uid.into());
        }
        if let Some(gid) = node.meta.gid {
            header.set_uid(gid.into());
        }
        if let Some(user) = &node.meta.user {
            header.set_username(user)?;
        }
        if let Some(group) = &node.meta.group {
            header.set_groupname(group)?;
        }
        if let Some(mtime) = node.meta.mtime {
            header.set_mtime(mtime.timestamp().try_into().unwrap_or_default());
        }

        // handle special files
        if node.is_symlink() {
            header.set_link_name(node.node_type.to_link())?;
        }
        match node.node_type {
            NodeType::Dev { device } | NodeType::Chardev { device } => {
                header.set_device_minor(device as u32)?;
                header.set_device_major((device << 32) as u32)?;
            }
            _ => {}
        }

        if node.is_file() {
            // write file content if this is a regular file
            let open_file = OpenFileReader {
                repo,
                open_file: repo.open_file(&node)?,
                offset: 0,
            };
            ar.append_data(&mut header, path, open_file)?;
        } else {
            let data: &[u8] = &[];
            ar.append_data(&mut header, path, data)?;
        }
    }
    // finish writing
    _ = ar.into_inner()?;
    Ok(())
}

fn dump_zip(
    repo: &CliIndexedRepo,
    node: &Node,
    w: &mut impl Write,
    ls_opts: &LsOptions,
) -> Result<()> {
    let w = SeekWriter {
        write: w,
        cursor: Cursor::new(Vec::new()),
        written: 0,
    };
    let mut zip = ZipWriter::new(w);
    zip.set_flush_on_finish_file(true);
    write_zip_contents(repo, node, &mut zip, ls_opts)?;
    let mut inner = zip.finish()?;
    inner.flush()?;
    Ok(())
}

fn write_zip_to_file(
    repo: &CliIndexedRepo,
    node: &Node,
    file: &mut (impl Write + Seek),
    ls_opts: &LsOptions,
) -> Result<()> {
    let mut zip = ZipWriter::new(file);
    write_zip_contents(repo, node, &mut zip, ls_opts)?;
    let _ = zip.finish()?;
    Ok(())
}

fn write_zip_contents(
    repo: &CliIndexedRepo,
    node: &Node,
    zip: &mut ZipWriter<impl Write + Seek>,
    ls_opts: &LsOptions,
) -> Result<()> {
    for item in repo.ls(node, ls_opts)? {
        let (path, node) = item?;

        let mut options = SimpleFileOptions::default();
        if let Some(mode) = node.meta.mode {
            // TODO: this is some go-mapped mode, but lower bits are the standard unix mode bits -> is this ok?
            options = options.unix_permissions(mode);
        }
        if let Some(mtime) = node.meta.mtime {
            options =
                options.last_modified_time(mtime.naive_local().try_into().unwrap_or_default());
        }
        if node.is_file() {
            zip.start_file_from_path(path, options)?;
            repo.dump(&node, zip)?;
        } else {
            zip.add_directory_from_path(path, options)?;
        }
    }
    Ok(())
}

struct SeekWriter<W> {
    write: W,
    cursor: Cursor<Vec<u8>>,
    written: u64,
}

impl<W> Read for SeekWriter<W> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.cursor.read(buf)
    }
}

impl<W: Write> Write for SeekWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.cursor.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        _ = self.cursor.seek(SeekFrom::Start(0))?;
        let n = copy(&mut self.cursor, &mut self.write)?;
        _ = self.cursor.seek(SeekFrom::Start(0))?;
        self.cursor.get_mut().clear();
        self.cursor.get_mut().shrink_to(1_000_000);
        self.written += n;
        Ok(())
    }
}

impl<W> Seek for SeekWriter<W> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match pos {
            SeekFrom::Start(n) => self.cursor.seek(SeekFrom::Start(n - self.written)),
            pos => self.cursor.seek(pos),
        }
    }
    fn stream_position(&mut self) -> std::io::Result<u64> {
        Ok(self.written + self.cursor.stream_position()?)
    }
}

struct OpenFileReader<'a> {
    repo: &'a CliIndexedRepo,
    open_file: OpenFile,
    offset: usize,
}

impl Read for OpenFileReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let data = self
            .repo
            .read_file_at(&self.open_file, self.offset, buf.len())
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
        let n = data.len();
        buf[..n].copy_from_slice(&data);
        self.offset += n;
        Ok(n)
    }
}
