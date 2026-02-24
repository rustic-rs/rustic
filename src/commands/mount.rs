//! `mount` subcommand

// ignore markdown clippy lints as we use doc-comments to generate clap help texts
#![allow(clippy::doc_markdown)]

mod fusefs;
use fusefs::FuseFS;

use abscissa_core::{
    Command, FrameworkError, FrameworkErrorKind::ParseError, Runnable, Shutdown, config::Override,
};
use anyhow::{Result, bail};
use clap::Parser;
use conflate::{Merge, MergeFrom};
use fuse_mt::{FuseMT, mount};
use log::info;
use rustic_core::vfs::{FilePolicy, IdenticalSnapshot, Latest, Vfs};
use std::{ffi::OsStr, path::PathBuf};

use crate::{
    Application, RUSTIC_APP, RusticConfig,
    repository::{IndexedRepo, get_filtered_snapshots},
    status_err,
};

#[derive(Clone, Debug, Default, Command, Parser, Merge, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct MountCmd {
    /// The path template to use for snapshots. {id}, {id_long}, {time}, {username}, {hostname}, {label}, {tags}, {backup_start}, {backup_end} are replaced. [default: "[{hostname}]/[{label}]/{time}"]
    #[clap(long)]
    #[merge(strategy=conflate::option::overwrite_none)]
    path_template: Option<String>,

    /// The time template to use to display times in the path template. See https://pubs.opengroup.org/onlinepubs/009695399/functions/strftime.html for format options. [default: "%Y-%m-%d_%H-%M-%S"]
    #[clap(long)]
    #[merge(strategy=conflate::option::overwrite_none)]
    time_template: Option<String>,

    /// Don't allow other users to access the mount point (else options allow_other,default_permissions are set)
    #[clap(short, long)]
    #[merge(strategy=conflate::bool::overwrite_false)]
    exclusive: bool,

    /// How to handle access to files. [default: "forbidden" for hot/cold repositories, else "read"]
    #[clap(long)]
    #[merge(strategy=conflate::option::overwrite_none)]
    file_access: Option<FilePolicy>,

    /// The mount point to use
    #[clap(value_name = "PATH")]
    #[merge(strategy=conflate::option::overwrite_none)]
    mount_point: Option<PathBuf>,

    /// Specify directly which snapshot/path to mount
    ///
    /// Snapshot can be identified the following ways: "01a2b3c4" or "latest" or "latest~N" (N >= 0)
    #[clap(value_name = "SNAPSHOT[:PATH]")]
    #[merge(strategy=conflate::option::overwrite_none)]
    snapshot_path: Option<String>,

    /// Other options to use for mount
    #[clap(short, long = "option", value_name = "OPTION")]
    #[merge(strategy = conflate::vec::overwrite_empty)]
    options: Vec<String>,
}

impl Override<RusticConfig> for MountCmd {
    // Process the given command line options, overriding settings from
    // a configuration file using explicit flags taken from command-line
    // arguments.
    fn override_config(&self, mut config: RusticConfig) -> Result<RusticConfig, FrameworkError> {
        // Merge by precedence, cli <- config <- default
        let self_config = self
            .clone()
            .merge_from(config.mount)
            .merge_from(Self::with_default_config());

        // Other values
        if self_config.mount_point.is_none() {
            return Err(ParseError
                .context("Please specify a valid mount point!")
                .into());
        }

        // rewrite the "mount" section in the config file
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
    fn with_default_config() -> Self {
        Self {
            path_template: Some(String::from("[{hostname}]/[{label}]/{time}")),
            time_template: Some(String::from("%Y-%m-%d_%H-%M-%S")),
            options: Vec::new(),
            ..Default::default()
        }
    }

    fn inner_run(&self, repo: IndexedRepo) -> Result<()> {
        let config = RUSTIC_APP.config();

        // We have merged the config file, the command line options, and the
        // default values into a single struct. Now we can use the values.
        // If a value is missing, we can return an error.
        let Some(path_template) = config.mount.path_template.clone() else {
            bail!("Please specify a path template!");
        };

        let Some(time_template) = config.mount.time_template.clone() else {
            bail!("Please specify a time template!");
        };

        let Some(mount_point) = config.mount.mount_point.clone() else {
            bail!("Please specify a mount point!");
        };

        let vfs = if let Some(snap) = &config.mount.snapshot_path {
            let node =
                repo.node_from_snapshot_path(snap, |sn| config.snapshot_filter.matches(sn))?;
            Vfs::from_dir_node(&node)
        } else {
            let snapshots = get_filtered_snapshots(&repo)?;
            Vfs::from_snapshots(
                snapshots,
                &path_template,
                &time_template,
                Latest::AsLink,
                IdenticalSnapshot::AsLink,
            )?
        };

        // Prepare the mount options
        let mut mount_options = config.mount.options.clone();

        mount_options.push(format!("fsname=rusticfs:{}", repo.config().id));

        if !config.mount.exclusive {
            mount_options
                .extend_from_slice(&["allow_other".to_string(), "default_permissions".to_string()]);
        }

        let file_access = config.mount.file_access.as_ref().map_or_else(
            || {
                if repo.config().is_hot == Some(true) {
                    FilePolicy::Forbidden
                } else {
                    FilePolicy::Read
                }
            },
            |s| *s,
        );

        let fs = FuseMT::new(FuseFS::new(repo, vfs, file_access), 1);

        // Sort and deduplicate options
        mount_options.sort_unstable();
        mount_options.dedup();

        // join options into a single comma-delimited string and prepent "-o "
        // this should be parsed just fine by fuser, here
        // https://github.com/cberner/fuser/blob/9f6ced73a36f1d99846e28be9c5e4903939ee9d5/src/mnt/mount_options.rs#L157
        let opt_string = format!("-o{}", mount_options.join(","));

        info!(
            "mounting {}, press Ctrl-C to cancel...",
            mount_point.display()
        );
        mount(fs, mount_point, &[OsStr::new(&opt_string)])?;

        Ok(())
    }
}
