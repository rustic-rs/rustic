//! `mount` subcommand

// ignore markdown clippy lints as we use doc-comments to generate clap help texts
#![allow(clippy::doc_markdown)]

mod fusefs;
use fusefs::FuseFS;

use std::{ffi::OsStr, path::PathBuf};

use crate::{repository::CliIndexedRepo, status_err, Application, RusticConfig, RUSTIC_APP};

use abscissa_core::{config::Override, Command, FrameworkError, Runnable, Shutdown};
use anyhow::{anyhow, Result};
use conflate::Merge;
use fuse_mt::{mount, FuseMT};
use rustic_core::vfs::{FilePolicy, IdenticalSnapshot, Latest, Vfs};
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
    exclusive: bool,

    /// How to handle access to files. [default: "forbidden" for hot/cold repositories, else "read"]
    #[clap(long)]
    #[merge(strategy=conflate::option::overwrite_none)]
    file_access: Option<String>,

    /// The mount point to use
    #[clap(value_name = "PATH")]
    #[merge(strategy=conflate::option::overwrite_none)]
    mount_point: Option<PathBuf>,

    /// Specify directly which snapshot/path to mount
    #[clap(value_name = "SNAPSHOT[:PATH]")]
    #[merge(strategy=conflate::option::overwrite_none)]
    snapshot_path: Option<String>,

    /// Other options to use for mount
    #[clap(skip)]
    options: MountOpts,
}

#[derive(Clone, Debug, Serialize, Deserialize, Merge)]
pub(crate) struct MountOpts(#[merge(strategy = conflate::vec::append)] pub(crate) Vec<String>);

impl Default for MountOpts {
    fn default() -> Self {
        Self(vec![String::from("kernel_cache")])
    }
}

impl Override<RusticConfig> for MountCmd {
    // Process the given command line options, overriding settings from
    // a configuration file using explicit flags taken from command-line
    // arguments.
    fn override_config(&self, mut config: RusticConfig) -> Result<RusticConfig, FrameworkError> {
        let mut self_config = self.clone();
        // merge "mount" section from config file, if given
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
        let mount_point = config
            .mount
            .mount_point
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

        let mut options = self.options.0.clone();

        options.extend_from_slice(&[format!("fsname=rusticfs:{}", repo.config().id)]);

        if !config.mount.exclusive {
            options
                .extend_from_slice(&["allow_other".to_string(), "default_permissions".to_string()]);
        }

        let file_access = config.mount.file_access.as_ref().map_or_else(
            || {
                if repo.config().is_hot == Some(true) {
                    Ok(FilePolicy::Forbidden)
                } else {
                    Ok(FilePolicy::Read)
                }
            },
            |s| s.parse(),
        )?;

        let fs = FuseMT::new(FuseFS::new(repo, vfs, file_access), 1);

        // Sort and deduplicate options
        options.sort_unstable();
        options.dedup();

        // join options into a single comma-delimited string and prepent "-o "
        // this should be parsed just fine by fuser, here
        // https://github.com/cberner/fuser/blob/9f6ced73a36f1d99846e28be9c5e4903939ee9d5/src/mnt/mount_options.rs#L157
        let opt_string = format!("-o {}", options.join(","));
        let options = OsStr::new(&opt_string);

        mount(fs, mount_point, &[options])?;

        Ok(())
    }
}
