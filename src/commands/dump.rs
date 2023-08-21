//! `dump` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{commands::open_repository, status_err, Application, RUSTIC_APP};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;

/// `dump` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct DumpCmd {
    /// file from snapshot to dump
    #[clap(value_name = "SNAPSHOT[:PATH]")]
    snap: String,
}

impl Runnable for DumpCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl DumpCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();

        let repo = open_repository(&config)?.to_indexed()?;
        let node =
            repo.node_from_snapshot_path(&self.snap, |sn| config.snapshot_filter.matches(sn))?;

        let mut stdout = std::io::stdout();
        repo.dump(&node, &mut stdout)?;

        Ok(())
    }
}
