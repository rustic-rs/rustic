//! `config` subcommand

pub mod set;
pub mod show;

use crate::commands::config::{set::SetConfigCmd, show::ShowConfigCmd};

use abscissa_core::{Command, Runnable};

/// `config` subcommand
#[derive(clap::Parser, Command, Runnable, Debug)]
pub(crate) struct ConfigCmd {
    #[clap(subcommand)]
    cmd: ConfigSubCmd,
}

#[derive(clap::Subcommand, Debug, Runnable)]
enum ConfigSubCmd {
    /// Set the configuration
    Set(SetConfigCmd),
    /// Show the configuration which has been read from the config file(s)
    Show(ShowConfigCmd),
}
