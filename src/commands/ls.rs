//! `ls` subcommand

use std::path::Path;

use crate::{commands::open_repository_indexed, status_err, Application, RUSTIC_APP};

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
    #[clap(long, short = 's', conflicts_with = "json")]
    summary: bool,

    /// show long listing
    #[clap(long, short = 'l', conflicts_with = "json")]
    long: bool,

    /// show listing in json
    #[clap(long, conflicts_with_all = ["summary", "long"])]
    json: bool,

    /// show uid/gid instead of user/group
    #[clap(long, long("numeric-uid-gid"))]
    numeric_id: bool,

    /// Listing options
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

/// Sumary of a ls command
///
/// This struct is used to print a summary of the ls command.
#[derive(Default)]
pub struct Summary {
    pub files: usize,
    pub size: u64,
    pub dirs: usize,
}

impl Summary {
    /// Update the summary with the node
    ///
    /// # Arguments
    ///
    /// * `node` - the node to update the summary with
    pub fn update(&mut self, node: &Node) {
        if node.is_dir() {
            self.dirs += 1;
        }
        if node.is_file() {
            self.files += 1;
            self.size += node.meta.size;
        }
    }
}

pub trait NodeLs {
    fn mode_str(&self) -> String;
    fn link_str(&self) -> String;
}

impl NodeLs for Node {
    fn mode_str(&self) -> String {
        format!(
            "{:>1}{:>9}",
            match self.node_type {
                NodeType::Dir => 'd',
                NodeType::Symlink { .. } => 'l',
                NodeType::Chardev { .. } => 'c',
                NodeType::Dev { .. } => 'b',
                NodeType::Fifo { .. } => 'p',
                NodeType::Socket => 's',
                _ => '-',
            },
            self.meta
                .mode
                .map(parse_permissions)
                .unwrap_or_else(|| "?????????".to_string())
        )
    }
    fn link_str(&self) -> String {
        if let NodeType::Symlink { .. } = &self.node_type {
            ["->", &self.node_type.to_link().to_string_lossy()].join(" ")
        } else {
            String::new()
        }
    }
}

impl LsCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();
        let repo = open_repository_indexed(&config.repository)?;

        let node =
            repo.node_from_snapshot_path(&self.snap, |sn| config.snapshot_filter.matches(sn))?;

        // recursive if standard if we specify a snapshot without dirs. In other cases, use the parameter `recursive`
        let mut ls_opts = self.ls_opts.clone();
        ls_opts.recursive = !self.snap.contains(':') || ls_opts.recursive;

        let mut summary = Summary::default();

        if self.json {
            print!("[");
        }

        let mut first_item = true;
        for item in repo.ls(&node, &ls_opts)? {
            let (path, node) = item?;
            summary.update(&node);
            if self.json {
                if !first_item {
                    print!(",");
                }
                print!("{}", serde_json::to_string(&path)?);
            } else if self.long {
                print_node(&node, &path, self.numeric_id);
            } else {
                println!("{}", path.display());
            }
            first_item = false;
        }

        if self.json {
            println!("]");
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

/// Print node in format similar to unix `ls`
///
/// # Arguments
///
/// * `node` - the node to print
/// * `path` - the path of the node
pub fn print_node(node: &Node, path: &Path, numeric_uid_gid: bool) {
    println!(
        "{:>10} {:>8} {:>8} {:>9} {:>17} {path:?} {}",
        node.mode_str(),
        if numeric_uid_gid {
            node.meta.uid.map(|uid| uid.to_string())
        } else {
            node.meta.user.clone()
        }
        .unwrap_or_else(|| "?".to_string()),
        if numeric_uid_gid {
            node.meta.gid.map(|uid| uid.to_string())
        } else {
            node.meta.group.clone()
        }
        .unwrap_or_else(|| "?".to_string()),
        node.meta.size,
        node.meta
            .mtime
            .map(|t| t.format("%_d %b %Y %H:%M").to_string())
            .unwrap_or_else(|| "?".to_string()),
        node.link_str(),
    );
}

/// Convert permissions into readable format
fn parse_permissions(mode: u32) -> String {
    let user = triplet(mode, S_IRUSR, S_IWUSR, S_IXUSR);
    let group = triplet(mode, S_IRGRP, S_IWGRP, S_IXGRP);
    let other = triplet(mode, S_IROTH, S_IWOTH, S_IXOTH);
    [user, group, other].join("")
}

/// Create a triplet of permissions
///
/// # Arguments
///
/// * `mode` - the mode to convert
/// * `read` - the read bit
/// * `write` - the write bit
/// * `execute` - the execute bit
///
/// # Returns
///
/// The triplet of permissions as a string
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
