//! `docs` subcommand

use abscissa_core::{status_err, Application, Command, Runnable, Shutdown};
use anyhow::Result;

use crate::{
    application::constants::{RUSTIC_CONFIG_DOCS_URL, RUSTIC_DEV_DOCS_URL, RUSTIC_DOCS_URL},
    RUSTIC_APP,
};

/// Opens the documentation in the default browser.
///
/// # Note
///
/// If no flag is set, the user documentation will be opened.
/// If the `dev` flag is set, the development documentation will be opened.
/// If the `config` flag is set, the configuration documentation will be opened.
#[derive(Clone, Command, Default, Debug, clap::Parser)]
pub struct DocsCmd {
    /// Open the development documentation
    #[clap(long, short)]
    dev: bool,

    /// Open the config documentation
    #[clap(long, short)]
    config: bool,
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
        let user_string = if self.dev && self.config {
            // If both flags are set, open the development documentation
            open::that(RUSTIC_DEV_DOCS_URL)?;
            open::that(RUSTIC_CONFIG_DOCS_URL)?;

            format!(
                "Opening the development documentation at {RUSTIC_DEV_DOCS_URL} and the configuration documentation at {RUSTIC_CONFIG_DOCS_URL}"
            )
        } else if self.config {
            // Open the config documentation
            open::that(RUSTIC_CONFIG_DOCS_URL)?;

            format!("Opening the configuration documentation at {RUSTIC_CONFIG_DOCS_URL}")
        } else if self.dev {
            // Open the development documentation
            open::that(RUSTIC_DEV_DOCS_URL)?;

            format!("Opening the development documentation at {RUSTIC_DEV_DOCS_URL}")
        } else {
            // Open the user documentation
            open::that(RUSTIC_DOCS_URL)?;

            format!("Opening the user documentation at {RUSTIC_DOCS_URL}")
        };

        println!("{user_string}");

        Ok(())
    }
}
