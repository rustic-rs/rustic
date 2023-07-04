//! `init` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use abscissa_core::{status_err, Command, Runnable, Shutdown};
use anyhow::{bail, Result};

use crate::{Application, RUSTIC_APP};

use dialoguer::Password;

use rustic_core::{
    random_poly, ConfigFile, DecryptBackend, DecryptWriteBackend, FileType, Id, KeyOpts,
    ReadBackend, Repository, WriteBackend,
};

use crate::commands::config::ConfigOpts;

/// `init` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct InitCmd {
    #[clap(flatten, next_help_heading = "Key options")]
    key_opts: KeyOpts,

    #[clap(flatten, next_help_heading = "Config options")]
    config_opts: ConfigOpts,
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

        let config_ids = repo.be.list(FileType::Config)?;

        let password = repo.password()?;

        if !config_ids.is_empty() {
            bail!("Config file already exists. Aborting.");
        }

        // Create config first to allow catching errors from here without writing anything
        let repo_id = Id::random();
        let chunker_poly = random_poly()?;
        let mut config = ConfigFile::new(2, repo_id, chunker_poly);
        self.config_opts.apply(&mut config)?;

        save_config(config, &repo, &self.key_opts, password)?;

        Ok(())
    }
}

pub(crate) fn save_config<P, S>(
    mut config: ConfigFile,
    repo: &Repository<P, S>,
    key_opts: &KeyOpts,
    password: Option<String>,
) -> Result<()> {
    let pass = password.unwrap_or_else(|| {
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

    repo.be.create()?;
    let (key, id) = repo.init_key(&pass, key_opts)?;
    println!("key {id} successfully added.");

    // save config
    let dbe = DecryptBackend::new(&repo.be, key);
    config.is_hot = None;
    _ = dbe.save_file(&config)?;

    if let Some(be_hot) = &repo.be_hot {
        let dbe = DecryptBackend::new(be_hot, key);
        config.is_hot = Some(true);
        _ = dbe.save_file(&config)?;
    }
    println!("repository {} successfully created.", config.id);

    Ok(())
}
