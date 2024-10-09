//! `config` subcommand

use crate::{status_err, Application, RUSTIC_APP};

use abscissa_core::{Command, Runnable, Shutdown};

use anyhow::Result;

use rustic_core::ConfigOptions;

/// `config` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct ConfigCmd {
    /// Config options
    #[clap(flatten)]
    config_opts: ConfigOptions,
}

impl Runnable for ConfigCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl ConfigCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();

        let changed = config
            .repository
            .run_open(|repo| Ok(repo.apply_config(&self.config_opts)?))?;

        if changed {
            println!("saved new config");
        } else {
            println!("config is unchanged");
        }

        Ok(())
    }
}
