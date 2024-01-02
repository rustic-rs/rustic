//! `show` subcommand

use crate::{Application, RUSTIC_APP};

use abscissa_core::{Command, Runnable};

/// `show` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct ShowConfigCmd {}

impl Runnable for ShowConfigCmd {
    fn run(&self) {
        let config = RUSTIC_APP.config();
        let toml_string = toml::to_string(&config).unwrap();
        println!("{toml_string}");
    }
}
