//! `ls` subcommand

use std::path::Path;

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{commands::open_repository, status_err, Application, RUSTIC_APP};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;

use rustic_core::{
    repofile::{Node, NodeType},
    LsOptions,
};

mod constants {
    // constants from man page inode(7)
    pub(super) const S_IRUSR: u32 = 0o400; //   owner has read permission
    pub(super) const S_IWUSR: u32 = 0o200; //   owner has write permission
    pub(super) const S_IXUSR: u32 = 0o100; //   owner has execute permission

    pub(super) const S_IRGRP: u32 = 0o040; //   group has read permission
    pub(super) const S_IWGRP: u32 = 0o020; //   group has write permission
    pub(super) const S_IXGRP: u32 = 0o010; //   group has execute permission

    pub(super) const S_IROTH: u32 = 0o004; //   others have read permission
    pub(super) const S_IWOTH: u32 = 0o002; //   others have write permission
    pub(super) const S_IXOTH: u32 = 0o001; //   others have execute permission
}
use constants::{S_IRGRP, S_IROTH, S_IRUSR, S_IWGRP, S_IWOTH, S_IWUSR, S_IXGRP, S_IXOTH, S_IXUSR};

/// `ls` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct LsCmd {
    /// Snapshot/path to list
    #[clap(value_name = "SNAPSHOT[:PATH]")]
    snap: String,

    /// show summary
    #[clap(long, short = 's')]
    summary: bool,

    /// show long listing
    #[clap(long, short = 'l')]
    long: bool,

    #[clap(flatten)]
    ls_opts: LsOptions,
}

impl Runnable for LsCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

#[derive(Default)]
struct Summary {
    files: usize,
    size: u64,
    dirs: usize,
}

impl Summary {
    fn update(&mut self, node: &Node) {
        if node.is_dir() {
            self.dirs += 1;
        }
        if node.is_file() {
            self.files += 1;
            self.size += node.meta.size;
        }
    }
}

impl LsCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();

        let repo = open_repository(&config)?.to_indexed()?;

        let node =
            repo.node_from_snapshot_path(&self.snap, |sn| config.snapshot_filter.matches(sn))?;

        // recursive if standard if we specify a snapshot without dirs. In other cases, use the parameter `recursive`
        let mut ls_opts = self.ls_opts.clone();
        ls_opts.recursive = !self.snap.contains(':') || ls_opts.recursive;

        let mut summary = Summary::default();

        for item in repo.ls(&node, &ls_opts)? {
            let (path, node) = item?;
            summary.update(&node);
            if self.long {
                print_node(&node, &path);
            } else {
                println!("{path:?} ");
            }
        }

        if self.summary {
            println!(
                "total: {} dirs, {} files, {} bytes",
                summary.dirs, summary.files, summary.size
            );
        }

        Ok(())
    }
}

// print node in format similar to unix `ls`
fn print_node(node: &Node, path: &Path) {
    println!(
        "{:>1}{:>9} {:>8} {:>8} {:>9} {:>12} {path:?} {}",
        match node.node_type {
            NodeType::Dir => 'd',
            NodeType::Symlink { .. } => 'l',
            NodeType::Chardev { .. } => 'c',
            NodeType::Dev { .. } => 'b',
            NodeType::Fifo { .. } => 'p',
            NodeType::Socket => 's',
            _ => '-',
        },
        node.meta
            .mode
            .map(parse_permissions)
            .unwrap_or_else(|| "?????????".to_string()),
        node.meta.user.clone().unwrap_or_else(|| "?".to_string()),
        node.meta.group.clone().unwrap_or_else(|| "?".to_string()),
        node.meta.size,
        node.meta
            .mtime
            .map(|t| t.format("%_d %b %H:%M").to_string())
            .unwrap_or_else(|| "?".to_string()),
        if let NodeType::Symlink { .. } = &node.node_type {
            ["->", &node.node_type.to_link().to_string_lossy()].join(" ")
        } else {
            String::new()
        }
    );
}

// helper fn to put permissions in readable format
fn parse_permissions(mode: u32) -> String {
    let user = triplet(mode, S_IRUSR, S_IWUSR, S_IXUSR);
    let group = triplet(mode, S_IRGRP, S_IWGRP, S_IXGRP);
    let other = triplet(mode, S_IROTH, S_IWOTH, S_IXOTH);
    [user, group, other].join("")
}

// helper fn to put permissions in readable format
fn triplet(mode: u32, read: u32, write: u32, execute: u32) -> String {
    match (mode & read, mode & write, mode & execute) {
        (0, 0, 0) => "---",
        (_, 0, 0) => "r--",
        (0, _, 0) => "-w-",
        (0, 0, _) => "--x",
        (_, 0, _) => "r-x",
        (_, _, 0) => "rw-",
        (0, _, _) => "-wx",
        (_, _, _) => "rwx",
    }
    .to_string()
}
