//! `check` subcommand

use crate::{commands::open_repository, status_err, Application, RUSTIC_APP};

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
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl CheckCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();
        let repo = open_repository(&config.repository)?;

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
