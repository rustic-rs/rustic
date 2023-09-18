//! `show-config` subcommand

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
