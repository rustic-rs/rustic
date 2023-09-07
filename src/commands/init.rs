//! `init` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use abscissa_core::{status_err, Command, Runnable, Shutdown};
use anyhow::{bail, Result};

use crate::{Application, RUSTIC_APP};

use dialoguer::Password;

use rustic_core::{ConfigOptions, KeyOptions, Repository};

/// `init` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct InitCmd {
    #[clap(flatten, next_help_heading = "Key options")]
    key_opts: KeyOptions,

    #[clap(flatten, next_help_heading = "Config options")]
    config_opts: ConfigOptions,
}

impl Runnable for InitCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl InitCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();

        let po = config.global.progress_options;
        let repo = Repository::new_with_progress(&config.repository, po)?;

        // Note: This is again checked in repo.init_with_password(), however we want to inform
        // users before they are prompted to enter a password
        if repo.config_id()?.is_some() {
            bail!("Config file already exists. Aborting.");
        }
        init(repo, &self.key_opts, &self.config_opts)
    }
}

pub(crate) fn init<P, S>(
    repo: Repository<P, S>,
    key_opts: &KeyOptions,
    config_opts: &ConfigOptions,
) -> Result<()> {
    let pass = repo.password()?.unwrap_or_else(|| {
        match Password::new()
            .with_prompt("enter password for new key")
            .allow_empty_password(true)
            .with_confirmation("confirm password", "passwords do not match")
            .interact()
        {
            Ok(it) => it,
            Err(err) => {
                status_err!("{}", err);
                RUSTIC_APP.shutdown(Shutdown::Crash);
            }
        }
    });

    let _ = repo.init_with_password(&pass, key_opts, config_opts)?;

    Ok(())
}
