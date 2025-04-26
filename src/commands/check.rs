//! `check` subcommand

use crate::{Application, RUSTIC_APP, repository::CliOpenRepo, status_err};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use rustic_core::{CheckOptions, SnapshotGroupCriterion};

/// `check` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct CheckCmd {
    /// Snapshots to check. If none is given, use filter options to filter from all snapshots
    #[clap(value_name = "ID")]
    ids: Vec<String>,

    /// Check options
    #[clap(flatten)]
    opts: CheckOptions,
}

impl Runnable for CheckCmd {
    fn run(&self) {
        if let Err(err) = RUSTIC_APP
            .config()
            .repository
            .run_open(|repo| self.inner_run(repo))
        {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl CheckCmd {
    fn inner_run(&self, repo: CliOpenRepo) -> Result<()> {
        let config = RUSTIC_APP.config();

        let groups = repo.get_snapshot_group(&self.ids, SnapshotGroupCriterion::new(), |sn| {
            config.snapshot_filter.matches(sn)
        })?;
        let trees = groups
            .into_iter()
            .flat_map(|(_, snaps)| snaps)
            .map(|snap| snap.tree)
            .collect();
        repo.check_with_trees(self.opts, trees)?;
        Ok(())
    }
}
