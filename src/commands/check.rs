//! `check` subcommand

use crate::{
    Application, RUSTIC_APP,
    repository::{CliOpenRepo, get_global_grouped_snapshots},
    status_err,
};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use rustic_core::CheckOptions;

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
        let groups = get_global_grouped_snapshots(&repo, &self.ids)?;
        let trees = groups
            .into_iter()
            .flat_map(|(_, snaps)| snaps)
            .map(|snap| snap.tree)
            .collect();
        repo.check_with_trees(self.opts, trees)?.is_ok()?;
        Ok(())
    }
}
