//! `rewrite` subcommand

use crate::{
    Application, RUSTIC_APP,
    commands::snapshots::print_snapshots,
    repository::{CliIndexedRepo, CliOpenRepo, get_snapots_from_ids},
    status_err,
};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use log::info;

use rustic_core::{
    Excludes,
    repofile::{SnapshotFile, SnapshotModification},
};

/// `rewrite` subcommand
#[derive(clap::Parser, Command, Debug, Default)]
pub(crate) struct RewriteCmd {
    /// Snapshots to rewrite. If none is given, use filter to filter from all snapshots.
    ///
    /// Snapshot can be identified the following ways: "01a2b3c4" or "latest" or "latest~N" (N >= 0)
    #[clap(value_name = "ID")]
    pub ids: Vec<String>,

    /// rebuild summary even if no excludes are given
    #[clap(long)]
    pub rebuild_summary: bool,

    /// remove original snapshots
    #[clap(long)]
    pub forget: bool,

    #[clap(flatten, next_help_heading = "Snapshot options")]
    pub modification: SnapshotModification,

    #[clap(flatten, next_help_heading = "Exclude options")]
    pub excludes: Excludes,
}

impl Runnable for RewriteCmd {
    fn run(&self) {
        if self.excludes.is_empty() && !self.rebuild_summary {
            if let Err(err) = RUSTIC_APP
                .config()
                .repository
                .run_open(|repo| self.inner_run_open(repo))
            {
                status_err!("{}", err);
                RUSTIC_APP.shutdown(Shutdown::Crash);
            }
        } else if let Err(err) = RUSTIC_APP
            .config()
            .repository
            .run_indexed(|repo| self.inner_run_indexed(repo))
        {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        }
    }
}

impl RewriteCmd {
    fn inner_run_open(&self, repo: CliOpenRepo) -> Result<()> {
        let config = RUSTIC_APP.config();
        let snapshots = get_snapots_from_ids(&repo, &self.ids)?;

        let snaps = repo.rewrite_snapshots(
            snapshots,
            &self.modification,
            config.global.dry_run,
            self.forget,
        )?;

        self.output(snaps);

        Ok(())
    }

    fn inner_run_indexed(&self, repo: CliIndexedRepo) -> Result<()> {
        let config = RUSTIC_APP.config();
        let snapshots = get_snapots_from_ids(&repo, &self.ids)?;

        let snaps = repo.rewrite_snapshots_with_excludes(
            snapshots,
            &self.modification,
            &self.excludes,
            config.global.dry_run,
            self.forget,
        )?;

        self.output(snaps);

        Ok(())
    }

    fn output(&self, snaps: Vec<SnapshotFile>) {
        let config = RUSTIC_APP.config();
        if config.global.dry_run {
            println!("Would have rewritten the following snapshots:");
            print_snapshots(snaps, false, true);
        } else {
            info!("{} snapshots have been rewritten", snaps.len());
        }
    }
}
