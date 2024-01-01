//! `repair` subcommand

use crate::{commands::open_repository, status_err, Application, RUSTIC_APP};
use abscissa_core::{Command, Runnable, Shutdown};

use anyhow::Result;

use rustic_core::{RepairIndexOptions, RepairSnapshotsOptions};

/// `repair` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct RepairCmd {
    /// Subcommand to run
    #[clap(subcommand)]
    cmd: RepairSubCmd,
}

#[derive(clap::Subcommand, Debug, Runnable)]
enum RepairSubCmd {
    /// Repair the repository index
    Index(IndexSubCmd),
    /// Repair snapshots
    Snapshots(SnapSubCmd),
}

#[derive(Default, Debug, clap::Parser, Command)]
struct IndexSubCmd {
    /// Index repair options
    #[clap(flatten)]
    opts: RepairIndexOptions,
}

/// `repair snapshots` subcommand
#[derive(Default, Debug, clap::Parser, Command)]
struct SnapSubCmd {
    /// Snapshot repair options
    #[clap(flatten)]
    opts: RepairSnapshotsOptions,

    /// Snapshots to repair. If none is given, use filter to filter from all snapshots.
    #[clap(value_name = "ID")]
    ids: Vec<String>,
}

impl Runnable for RepairCmd {
    fn run(&self) {
        self.cmd.run();
    }
}

impl Runnable for IndexSubCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl IndexSubCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();
        let repo = open_repository(&config)?;
        repo.repair_index(&self.opts, config.global.dry_run)?;
        Ok(())
    }
}

impl Runnable for SnapSubCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl SnapSubCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();
        let repo = open_repository(&config)?.to_indexed()?;
        let snaps = if self.ids.is_empty() {
            repo.get_all_snapshots()?
        } else {
            repo.get_snapshots(&self.ids)?
        };
        repo.repair_snapshots(&self.opts, snaps, config.global.dry_run)?;
        Ok(())
    }
}
