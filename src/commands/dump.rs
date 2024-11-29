//! `dump` subcommand

use std::io::{Read, Write};

use crate::{repository::CliIndexedRepo, status_err, Application, RUSTIC_APP};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use log::warn;
use rustic_core::{
    repofile::{Node, NodeType},
    vfs::OpenFile,
    LsOptions,
};
use tar::{Builder, EntryType, Header};

/// `dump` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct DumpCmd {
    /// file from snapshot to dump
    #[clap(value_name = "SNAPSHOT[:PATH]")]
    snap: String,

    /// Listing options
    #[clap(flatten)]
    ls_opts: LsOptions,
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

        let mut stdout = std::io::stdout();
        if node.is_file() {
            repo.dump(&node, &mut stdout)?;
        } else {
            dump_tar(&repo, &node, &mut stdout, &self.ls_opts)?;
        }

        Ok(())
    }
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
