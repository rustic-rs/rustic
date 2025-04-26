//! `docs` subcommand

use abscissa_core::{Application, Command, Runnable, Shutdown, status_err};
use anyhow::Result;
use clap::Subcommand;

use crate::{
    RUSTIC_APP,
    application::constants::{RUSTIC_CONFIG_DOCS_URL, RUSTIC_DEV_DOCS_URL, RUSTIC_DOCS_URL},
};

#[derive(Command, Debug, Clone, Copy, Default, Subcommand, Runnable)]
enum DocsTypeSubcommand {
    #[default]
    /// Show the user documentation
    User,
    /// Show the development documentation
    Dev,
    /// Show the configuration documentation
    Config,
}

/// Opens the documentation in the default browser.
#[derive(Clone, Command, Default, Debug, clap::Parser)]
pub struct DocsCmd {
    #[clap(subcommand)]
    cmd: Option<DocsTypeSubcommand>,
}

impl Runnable for DocsCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl DocsCmd {
    fn inner_run(&self) -> Result<()> {
        let user_string = match self.cmd {
            // Default to user docs if no subcommand is provided
            Some(DocsTypeSubcommand::User) | None => {
                open::that(RUSTIC_DOCS_URL)?;
                format!("Opening the user documentation at {RUSTIC_DOCS_URL}")
            }
            Some(DocsTypeSubcommand::Dev) => {
                open::that(RUSTIC_DEV_DOCS_URL)?;
                format!("Opening the development documentation at {RUSTIC_DEV_DOCS_URL}")
            }
            Some(DocsTypeSubcommand::Config) => {
                open::that(RUSTIC_CONFIG_DOCS_URL)?;
                format!("Opening the configuration documentation at {RUSTIC_CONFIG_DOCS_URL}")
            }
        };

        println!("{user_string}");

        Ok(())
    }
}
