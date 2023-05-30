//! `show-config` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{Application, RUSTIC_APP};

use abscissa_core::{Command, Runnable};

/// `show-config` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct ShowConfigCmd {}

impl Runnable for ShowConfigCmd {
    fn run(&self) {
        let config = RUSTIC_APP.config();
        println!("{config:#?}");
    }
}
