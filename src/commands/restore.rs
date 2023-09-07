//! `restore` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{
    commands::open_repository, helpers::bytes_size_to_string, status_err, Application, RUSTIC_APP,
};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use log::info;

use rustic_core::{LocalDestination, LsOptions, RestoreOptions};

use crate::filtering::SnapshotFilter;

/// `restore` subcommand
#[allow(clippy::struct_excessive_bools)]
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct RestoreCmd {
    /// Snapshot/path to restore
    #[clap(value_name = "SNAPSHOT[:PATH]")]
    snap: String,

    /// Restore destination
    #[clap(value_name = "DESTINATION")]
    dest: String,

    #[clap(flatten)]
    opts: RestoreOptions,

    #[clap(flatten)]
    ls_opts: LsOptions,

    #[clap(
        flatten,
        next_help_heading = "Snapshot filter options (when using latest)"
    )]
    filter: SnapshotFilter,
}
impl Runnable for RestoreCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl RestoreCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();
        let dry_run = config.global.dry_run;
        let repo = open_repository(&config)?.to_indexed()?;

        let node =
            repo.node_from_snapshot_path(&self.snap, |sn| config.snapshot_filter.matches(sn))?;

        // for restore, always recurse into tree
        let mut ls_opts = self.ls_opts.clone();
        ls_opts.recursive = true;
        let ls = repo.ls(&node, &ls_opts)?;

        let dest = LocalDestination::new(&self.dest, true, !node.is_dir())?;

        let restore_infos = repo.prepare_restore(&self.opts, ls.clone(), &dest, dry_run)?;

        let fs = restore_infos.stats.files;
        println!(
            "Files:  {} to restore, {} unchanged, {} verified, {} to modify, {} additional",
            fs.restore, fs.unchanged, fs.verified, fs.modify, fs.additional
        );
        let ds = restore_infos.stats.dirs;
        println!(
            "Dirs:   {} to restore, {} to modify, {} additional",
            ds.restore, ds.modify, ds.additional
        );

        info!(
            "total restore size: {}",
            bytes_size_to_string(restore_infos.restore_size)
        );
        if restore_infos.matched_size > 0 {
            info!(
                "using {} of existing file contents.",
                bytes_size_to_string(restore_infos.matched_size)
            );
        }
        if restore_infos.restore_size == 0 {
            info!("all file contents are fine.");
        }

        if dry_run {
            repo.warm_up(restore_infos.to_packs().into_iter())?;
        } else {
            repo.restore(restore_infos, &self.opts, ls, &dest)?;
            println!("restore done.");
        }

        Ok(())
    }
}
