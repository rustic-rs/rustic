//! `restore` subcommand

use crate::{
    Application, RUSTIC_APP, helpers::bytes_size_to_string, repository::CliIndexedRepo, status_err,
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

    /// Restore options
    #[clap(flatten)]
    opts: RestoreOptions,

    /// List options
    #[clap(flatten)]
    ls_opts: LsOptions,

    /// Snapshot filter options (when using latest)
    #[clap(
        flatten,
        next_help_heading = "Snapshot filter options (when using latest)"
    )]
    filter: SnapshotFilter,
}
impl Runnable for RestoreCmd {
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

impl RestoreCmd {
    fn inner_run(&self, repo: CliIndexedRepo) -> Result<()> {
        let config = RUSTIC_APP.config();
        let dry_run = config.global.dry_run;

        let node =
            repo.node_from_snapshot_path(&self.snap, |sn| config.snapshot_filter.matches(sn))?;

        // for restore, always recurse into tree
        let mut ls_opts = self.ls_opts.clone();
        ls_opts.recursive = true;
        let ls = repo.ls(&node, &ls_opts)?;

        let dest = LocalDestination::new(&self.dest, true, !node.is_dir())?;

        let restore_infos = repo.prepare_restore(&self.opts, ls, &dest, dry_run)?;

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
            // save some memory
            let repo = repo.drop_data_from_index();

            let ls = repo.ls(&node, &ls_opts)?;
            repo.restore(restore_infos, &self.opts, ls, &dest)?;
            println!("restore done.");
        }

        Ok(())
    }
}
