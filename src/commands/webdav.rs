//! `mount` subcommand
use std::net::ToSocketAddrs;

use crate::{commands::open_repository, status_err, Application, RUSTIC_APP};
use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::{anyhow, Result};
use dav_server::{warp::dav_handler, DavHandler};
use rustic_core::vfs::{FilePolicy, IdenticalSnapshot, Latest, Vfs};

#[derive(clap::Parser, Command, Debug)]
pub(crate) struct WebDavCmd {
    /// Address to bind the webdav server to
    #[clap(long, value_name = "ADDRESS", default_value = "localhost:8000")]
    addr: String,

    /// The path template to use for snapshots. {id}, {id_long}, {time}, {username}, {hostname}, {label}, {tags}, {backup_start}, {backup_end} are replaced. [default: "[{hostname}]/[{label}]/{time}"]
    #[clap(long)]
    path_template: Option<String>,

    /// The time template to use to display times in the path template. See https://docs.rs/chrono/latest/chrono/format/strftime/index.html for format options. [default: "%Y-%m-%d_%H-%M-%S"]
    #[clap(long)]
    time_template: Option<String>,

    /// Use symlinks. This may not be supported by all WebDAV clients
    #[clap(long)]
    symlinks: bool,

    /// How to handle access to files. Default: "forbidden" for hot/cold repositories, else "read"
    #[clap(long)]
    file_access: Option<FilePolicy>,

    /// Specify directly which path to mount
    #[clap(value_name = "SNAPSHOT[:PATH]")]
    snap: Option<String>,
}

impl Runnable for WebDavCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl WebDavCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();
        let repo = open_repository(&config.repository)?.to_indexed()?;

        let file_access = self.file_access.unwrap_or_else(|| {
            if repo.config().is_hot == Some(true) {
                FilePolicy::Forbidden
            } else {
                FilePolicy::Read
            }
        });

        let path_template = self
            .path_template
            .clone()
            .unwrap_or_else(|| "[{hostname}]/[{label}]/{time}".to_string());
        let time_template = self
            .time_template
            .clone()
            .unwrap_or_else(|| "%Y-%m-%d_%H-%M-%S".to_string());

        let sn_filter = |sn: &_| config.snapshot_filter.matches(sn);

        let vfs = if let Some(snap) = &self.snap {
            let node = repo.node_from_snapshot_path(snap, sn_filter)?;
            Vfs::from_dirnode(node, file_access)
        } else {
            let snapshots = repo.get_matching_snapshots(sn_filter)?;
            let (latest, identical) = if self.symlinks {
                (Latest::AsLink, IdenticalSnapshot::AsLink)
            } else {
                (Latest::AsDir, IdenticalSnapshot::AsDir)
            };
            Vfs::from_snapshots(
                snapshots,
                path_template,
                time_template,
                latest,
                identical,
                file_access,
            )?
        };
        let addr = self
            .addr
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| anyhow!("no address given"))?;
        let dav_server = DavHandler::builder()
            .filesystem(vfs.into_webdav_fs(repo))
            .build_handler();

        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(async {
                warp::serve(dav_handler(dav_server)).run(addr).await;
            });

        Ok(())
    }
}
