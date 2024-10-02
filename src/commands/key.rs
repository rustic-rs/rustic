//! `key` subcommand

use crate::{commands::open_repository, status_err, Application, RUSTIC_APP};

use std::path::PathBuf;

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use dialoguer::Password;
use log::info;

use rustic_core::{CommandInput, KeyOptions, RepositoryOptions};

/// `key` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(super) struct KeyCmd {
    /// Subcommand to run
    #[clap(subcommand)]
    cmd: KeySubCmd,
}

#[derive(clap::Subcommand, Debug, Runnable)]
enum KeySubCmd {
    /// Add a new key to the repository
    Add(AddCmd),
}

#[derive(clap::Parser, Debug)]
pub(crate) struct AddCmd {
    /// New password
    #[clap(long)]
    pub(crate) new_password: Option<String>,

    /// File from which to read the new password
    #[clap(long)]
    pub(crate) new_password_file: Option<PathBuf>,

    /// Command to get the new password from
    #[clap(long)]
    pub(crate) new_password_command: Option<CommandInput>,

    /// Key options
    #[clap(flatten)]
    pub(crate) key_opts: KeyOptions,
}

impl Runnable for KeyCmd {
    fn run(&self) {
        self.cmd.run();
    }
}

impl Runnable for AddCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl AddCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();
        let repo = open_repository(&config.repository)?;

        // create new Repository options which just contain password information
        let mut pass_opts = RepositoryOptions::default();
        pass_opts.password = self.new_password.clone();
        pass_opts.password_file = self.new_password_file.clone();
        pass_opts.password_command = self.new_password_command.clone();

        let pass = pass_opts
            .evaluate_password()
            .map_err(|err| err.into())
            .transpose()
            .unwrap_or_else(|| -> Result<_> {
                Ok(Password::new()
                    .with_prompt("enter password for new key")
                    .allow_empty_password(true)
                    .with_confirmation("confirm password", "passwords do not match")
                    .interact()?)
            })?;

        let id = repo.add_key(&pass, &self.key_opts)?;
        info!("key {id} successfully added.");

        Ok(())
    }
}
