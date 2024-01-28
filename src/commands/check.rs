//! `check` subcommand

use crate::{commands::open_repository, status_err, Application, RUSTIC_APP};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use rustic_core::CheckOptions;

/// `check` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct CheckCmd {
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
        repo.check(self.opts)?;
        Ok(())
    }
}
