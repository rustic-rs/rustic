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
#[clap(group = clap::ArgGroup::new("documentation").multiple(false))]
pub struct DocsCmd {
    /// Open the user documentation
    #[clap(short, long, group = "documentation")]
    user: bool,

    /// Open the development documentation
    #[clap(short, long, group = "documentation")]
    dev: bool,

    /// Open the config documentation
    #[clap(short, long, group = "documentation")]
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
        let user_string = match (self.user, self.dev, self.config) {
            // Default: If no flag is set, open the user documentation
            (true, false, false) | (false, false, false) => {
                open::that(RUSTIC_DOCS_URL)?;
                format!("Opening the user documentation at {RUSTIC_DOCS_URL}")
            }
            (false, true, false) => {
                open::that(RUSTIC_DEV_DOCS_URL)?;
                format!("Opening the development documentation at {RUSTIC_DEV_DOCS_URL}")
            }
            (false, false, true) => {
                open::that(RUSTIC_CONFIG_DOCS_URL)?;
                format!("Opening the configuration documentation at {RUSTIC_CONFIG_DOCS_URL}")
            }
            _ => unreachable!("this should not be possible"),
        };

        println!("{user_string}");

        Ok(())
    }
}
