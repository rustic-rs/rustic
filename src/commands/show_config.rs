//! `show-config` subcommand

use crate::{Application, RUSTIC_APP, status_err};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use toml::to_string_pretty;

/// `show-config` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct ShowConfigCmd {}

impl Runnable for ShowConfigCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl ShowConfigCmd {
    fn inner_run(&self) -> Result<()> {
        let config = to_string_pretty(RUSTIC_APP.config().as_ref())?;
        println!("{config}");
        Ok(())
    }
}
