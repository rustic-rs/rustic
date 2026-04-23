use abscissa_core::{Command, Runnable};
use clap::Parser;

// `version` command
#[derive(Command, Debug, Parser)]
pub struct VersionCmd {}

impl Runnable for VersionCmd {
    // Print the version and exit
    fn run(&self) {
        // Use the existing version helper from the parent module
        println!("rustic {}", crate::commands::version());
    }
}
