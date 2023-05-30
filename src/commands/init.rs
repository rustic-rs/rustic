//! `init` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use abscissa_core::{status_err, Command, Runnable, Shutdown};
use anyhow::{bail, Result};

use crate::{commands::get_repository, Application, RUSTIC_APP};

use bytes::Bytes;
use rpassword::prompt_password;

use rustic_core::{
    hash, random_poly, ConfigFile, DecryptBackend, DecryptWriteBackend, FileType, Id, Key, KeyFile,
    ReadBackend, WriteBackend,
};

use crate::commands::{config::ConfigOpts, key::KeyOpts};

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

        let repo = get_repository(&config);

        let config_ids = repo.be.list(FileType::Config)?;

        let password = repo.password()?;

        let be = &repo.be;
        let hot_be = &repo.be_hot;

        if !config_ids.is_empty() {
            bail!("Config file already exists. Aborting.");
        }

        // Create config first to allow catching errors from here without writing anything
        let repo_id = Id::random();
        let chunker_poly = random_poly()?;
        let version = match self.config_opts.set_version {
            None => 2,
            Some(_) => 1, // will be changed later
        };
        let mut config = ConfigFile::new(version, repo_id, chunker_poly);
        self.config_opts.apply(&mut config)?;

        save_config(config, be, hot_be, self.key_opts.clone(), password)?;

        Ok(())
    }
}

pub(crate) fn save_config(
    mut config: ConfigFile,
    be: &impl WriteBackend,
    hot_be: &Option<impl WriteBackend>,
    key_opts: KeyOpts,
    password: Option<String>,
) -> Result<()> {
    // generate key
    let key = Key::new();

    let pass = password.map_or_else(
        || match prompt_password("enter password for new key: ") {
            Ok(it) => it,
            Err(err) => {
                status_err!("{}", err);
                RUSTIC_APP.shutdown(Shutdown::Crash);
            }
        },
        |pass| pass,
    );

    let keyfile = KeyFile::generate(
        key,
        &pass,
        key_opts.hostname,
        key_opts.username,
        key_opts.with_created,
    )?;
    let data: Bytes = serde_json::to_vec(&keyfile)?.into();
    let id = hash(&data);
    be.create()?;
    be.write_bytes(FileType::Key, &id, false, data)?;
    println!("key {id} successfully added.");

    // save config
    let dbe = DecryptBackend::new(be, key);
    config.is_hot = None;
    _ = dbe.save_file(&config)?;

    if let Some(hot_be) = hot_be {
        let dbe = DecryptBackend::new(hot_be, key);
        config.is_hot = Some(true);
        _ = dbe.save_file(&config)?;
    }
    println!("repository {} successfully created.", config.id);

    Ok(())
}
