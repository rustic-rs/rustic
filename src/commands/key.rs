//! `key` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{commands::open_repository, status_err, Application, RUSTIC_APP};

use std::path::PathBuf;

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use dialoguer::Password;
use log::info;

use rustic_core::{KeyOptions, Repository, RepositoryOptions};

/// `key` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(super) struct KeyCmd {
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
    /// File from which to read the new password
    #[clap(long)]
    pub(crate) new_password_file: Option<PathBuf>,

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

        let repo = open_repository(&config)?;

        // create new "artificial" repo using the given password options
        let repo_opts = RepositoryOptions {
            password_file: self.new_password_file.clone(),
            repository: Some(String::new()), // fake repository to make Repository::new() not bail
            ..Default::default()
        };
        let repo_newpass = Repository::new(&repo_opts)?;

        let pass = repo_newpass
            .password()
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
