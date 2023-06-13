//! `check` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{
    commands::{get_repository, open_repository},
    status_err, Application, RUSTIC_APP,
};

use abscissa_core::{Command, Runnable, Shutdown};

use rustic_core::CheckOpts;

/// `check` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct CheckCmd {
    #[clap(flatten)]
    opts: CheckOpts,
}

impl Runnable for CheckCmd {
    fn run(&self) {
        let config = RUSTIC_APP.config();
        let progress_options = config.global.progress_options;

        let repo = open_repository(get_repository(&config));
        if let Err(err) = repo.check(self.opts, &progress_options) {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}
