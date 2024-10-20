//! `mount` subcommand
mod fusefs;
use fusefs::FuseFS;

use std::{ffi::OsStr, path::PathBuf};

use crate::{repository::CliIndexedRepo, status_err, Application, RusticConfig, RUSTIC_APP};

use abscissa_core::{config::Override, Command, FrameworkError, Runnable, Shutdown};
use anyhow::{anyhow, Result};
use conflate::Merge;
use fuse_mt::{mount, FuseMT};
use rustic_core::vfs::{IdenticalSnapshot, Latest, Vfs};
use serde::{Deserialize, Serialize};

#[derive(Clone, Command, Default, Debug, clap::Parser, Serialize, Deserialize, Merge)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct MountCmd {
    /// The path template to use for snapshots. {id}, {id_long}, {time}, {username}, {hostname}, {label}, {tags}, {backup_start}, {backup_end} are replaced. [default: "[{hostname}]/[{label}]/{time}"]
    #[clap(long)]
    #[merge(strategy=conflate::option::overwrite_none)]
    path_template: Option<String>,

    /// The time template to use to display times in the path template. See https://docs.rs/chrono/latest/chrono/format/strftime/index.html for format options. [default: "%Y-%m-%d_%H-%M-%S"]
    #[clap(long)]
    #[merge(strategy=conflate::option::overwrite_none)]
    time_template: Option<String>,

    /// Don't allow other users to access the mount point
    #[clap(long)]
    #[merge(strategy=conflate::bool::overwrite_false)]
    no_allow_other: bool,

    /// The mount point to use
    #[clap(value_name = "PATH")]
    #[merge(strategy=conflate::option::overwrite_none)]
    mountpoint: Option<PathBuf>,

    /// Specify directly which snapshot/path to mount
    #[clap(value_name = "SNAPSHOT[:PATH]")]
    #[merge(strategy=conflate::option::overwrite_none)]
    snapshot_path: Option<String>,
}

impl Override<RusticConfig> for MountCmd {
    // Process the given command line options, overriding settings from
    // a configuration file using explicit flags taken from command-line
    // arguments.
    fn override_config(&self, mut config: RusticConfig) -> Result<RusticConfig, FrameworkError> {
        let mut self_config = self.clone();
        // merge "webdav" section from config file, if given
        self_config.merge(config.mount);
        config.mount = self_config;
        Ok(config)
    }
}

impl Runnable for MountCmd {
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

impl MountCmd {
    fn inner_run(&self, repo: CliIndexedRepo) -> Result<()> {
        let config = RUSTIC_APP.config();
        let mountpoint = config
            .mount
            .mountpoint
            .as_ref()
            .ok_or_else(|| anyhow!("please specify a mountpoint"))?;

        let path_template = config
            .mount
            .path_template
            .clone()
            .unwrap_or_else(|| "[{hostname}]/[{label}]/{time}".to_string());
        let time_template = config
            .mount
            .time_template
            .clone()
            .unwrap_or_else(|| "%Y-%m-%d_%H-%M-%S".to_string());

        let sn_filter = |sn: &_| config.snapshot_filter.matches(sn);
        let vfs = if let Some(snap_path) = &config.mount.snapshot_path {
            let node = repo.node_from_snapshot_path(snap_path, sn_filter)?;
            Vfs::from_dir_node(&node)
        } else {
            let snapshots = repo.get_matching_snapshots(sn_filter)?;
            Vfs::from_snapshots(
                snapshots,
                &path_template,
                &time_template,
                Latest::AsLink,
                IdenticalSnapshot::AsLink,
            )?
        };

        let name_opt = format!("fsname=rusticfs:{}", repo.config().id);
        let mut options = vec![
            OsStr::new("-o"),
            OsStr::new(&name_opt),
            OsStr::new("-o"),
            OsStr::new("kernel_cache"),
        ];

        if !config.mount.no_allow_other {
            options.extend_from_slice(&[
                OsStr::new("-o"),
                OsStr::new("allow_other"),
                OsStr::new("-o"),
                OsStr::new("default_permissions"),
            ]);
        }

        let fs = FuseMT::new(FuseFS::new(repo, vfs), 1);
        mount(fs, mountpoint, &options)?;

        Ok(())
    }
}
