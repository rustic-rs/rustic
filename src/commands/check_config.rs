//! `check-config` subcommand

use crate::{Application, RUSTIC_APP, status_err};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::{Result, bail};

/// `check-config` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct CheckConfigCmd {}

impl Runnable for CheckConfigCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl CheckConfigCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();
        if let Err(err) = config.backup.validate() {
            bail!("{err}");
        }

        println!("config ok");
        Ok(())
    }
}
