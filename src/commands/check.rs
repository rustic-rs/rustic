//! `check` subcommand

use crate::{status_err, Application, RUSTIC_APP};

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
        config
            .repository
            .run_open(|repo| Ok(repo.check(self.opts)?))
    }
}
