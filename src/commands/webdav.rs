//! `webdav` subcommand

// ignore markdown clippy lints as we use doc-comments to generate clap help texts
#![allow(clippy::doc_markdown)]

use std::{net::ToSocketAddrs, str::FromStr};

use crate::{commands::open_repository_indexed, status_err, Application, RusticConfig, RUSTIC_APP};
use abscissa_core::{config::Override, Command, FrameworkError, Runnable, Shutdown};
use anyhow::{anyhow, Result};
use dav_server::{warp::dav_handler, DavHandler};
use merge::Merge;
use serde::{Deserialize, Serialize};

use rustic_core::vfs::{FilePolicy, IdenticalSnapshot, Latest, Vfs};

#[derive(Clone, Command, Default, Debug, clap::Parser, Serialize, Deserialize, Merge)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct WebDavCmd {
    /// Address to bind the webdav server to. [default: "localhost:8000"]
    #[clap(long, value_name = "ADDRESS")]
    address: Option<String>,

    /// The path template to use for snapshots. {id}, {id_long}, {time}, {username}, {hostname}, {label}, {tags}, {backup_start}, {backup_end} are replaced. [default: "[{hostname}]/[{label}]/{time}"]
    #[clap(long)]
    path_template: Option<String>,

    /// The time template to use to display times in the path template. See https://docs.rs/chrono/latest/chrono/format/strftime/index.html for format options. [default: "%Y-%m-%d_%H-%M-%S"]
    #[clap(long)]
    time_template: Option<String>,

    /// Use symlinks. This may not be supported by all WebDAV clients
    #[clap(long)]
    #[merge(strategy = merge::bool::overwrite_false)]
    symlinks: bool,

    /// How to handle access to files. [default: "forbidden" for hot/cold repositories, else "read"]
    #[clap(long)]
    file_access: Option<String>,

    /// Specify directly which snapshot/path to serve
    #[clap(value_name = "SNAPSHOT[:PATH]")]
    snapshot_path: Option<String>,
}

impl Override<RusticConfig> for WebDavCmd {
    // Process the given command line options, overriding settings from
    // a configuration file using explicit flags taken from command-line
    // arguments.
    fn override_config(&self, mut config: RusticConfig) -> Result<RusticConfig, FrameworkError> {
        let mut self_config = self.clone();
        // merge "webdav" section from config file, if given
        self_config.merge(config.webdav);
        config.webdav = self_config;
        Ok(config)
    }
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
        let repo = open_repository_indexed(&config.repository)?;

        let path_template = config
            .webdav
            .path_template
            .clone()
            .unwrap_or_else(|| "[{hostname}]/[{label}]/{time}".to_string());
        let time_template = config
            .webdav
            .time_template
            .clone()
            .unwrap_or_else(|| "%Y-%m-%d_%H-%M-%S".to_string());

        let sn_filter = |sn: &_| config.snapshot_filter.matches(sn);

        let vfs = if let Some(snap) = &config.webdav.snapshot_path {
            let node = repo.node_from_snapshot_path(snap, sn_filter)?;
            Vfs::from_dir_node(&node)
        } else {
            let snapshots = repo.get_matching_snapshots(sn_filter)?;
            let (latest, identical) = if config.webdav.symlinks {
                (Latest::AsLink, IdenticalSnapshot::AsLink)
            } else {
                (Latest::AsDir, IdenticalSnapshot::AsDir)
            };
            Vfs::from_snapshots(snapshots, &path_template, &time_template, latest, identical)?
        };

        let addr = config
            .webdav
            .address
            .clone()
            .unwrap_or_else(|| "localhost:8000".to_string())
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| anyhow!("no address given"))?;

        let file_access = config.webdav.file_access.as_ref().map_or_else(
            || {
                if repo.config().is_hot == Some(true) {
                    Ok(FilePolicy::Forbidden)
                } else {
                    Ok(FilePolicy::Read)
                }
            },
            |s| FilePolicy::from_str(s),
        )?;

        let dav_server = DavHandler::builder()
            .filesystem(vfs.into_webdav_fs(repo, file_access))
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
