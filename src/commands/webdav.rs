//! `mount` subcommand
use std::net::SocketAddr;
use std::str::FromStr;

use crate::{commands::open_repository, status_err, Application, RUSTIC_APP};
use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use rustic_core::vfs::{IdenticalSnapshot, Latest, Vfs};
use webdav_handler::{warp::dav_handler, DavHandler};

#[derive(clap::Parser, Command, Debug)]
pub(crate) struct WebDavCmd {
    /// The path template to use for snapshots. {id}, {id_long}, {time}, {username}, {hostname}, {label}, {tags}, {backup_start}, {backup_end} are replaced. [default: "[{hostname}]/[{label}]/{time}"]
    #[clap(long)]
    path_template: Option<String>,

    /// The time template to use to display times in the path template. See https://docs.rs/chrono/latest/chrono/format/strftime/index.html for format options. [default: "%Y-%m-%d_%H-%M-%S"]
    #[clap(long)]
    time_template: Option<String>,

    /// Use symlinks. This may not be supported by all WebDAV clients
    #[clap(long)]
    symlinks: bool,

    /// Socket address to use
    #[clap(value_name = "ADDR")]
    socket: String,

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
            Vfs::from_dirnode(node)
        } else {
            let snapshots = repo.get_matching_snapshots(sn_filter)?;
            let (latest, identical) = if self.symlinks {
                (Latest::AsLink, IdenticalSnapshot::AsLink)
            } else {
                (Latest::AsDir, IdenticalSnapshot::AsDir)
            };
            Vfs::from_snapshots(snapshots, path_template, time_template, latest, identical)?
        };
        let addr = SocketAddr::from_str(&self.socket)?;
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
