//! `key` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{
    commands::{get_repository, open_repository},
    status_err, Application, RUSTIC_APP,
};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;

use std::{fs::File, io::BufReader};

use rpassword::{prompt_password, read_password_from_bufread};

use rustic_core::{hash, FileType, KeyFile, WriteBackend};

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
    pub(crate) new_password_file: Option<String>,

    #[clap(flatten)]
    pub(crate) key_opts: KeyOpts,
}

#[derive(clap::Parser, Debug, Clone)]
pub(crate) struct KeyOpts {
    /// Set 'hostname' in public key information
    #[clap(long)]
    pub(crate) hostname: Option<String>,

    /// Set 'username' in public key information
    #[clap(long)]
    pub(crate) username: Option<String>,

    /// Add 'created' date in public key information
    #[clap(long)]
    pub(crate) with_created: bool,
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

        let repo = open_repository(get_repository(&config));

        let be = &repo.dbe;
        let key = repo.key;

        let pass = self.new_password_file.as_ref().map_or_else(
            || match prompt_password("enter password for new key: ") {
                Ok(it) => it,
                Err(err) => {
                    status_err!("{}", err);
                    RUSTIC_APP.shutdown(Shutdown::Crash);
                }
            },
            |file| {
                let mut file = BufReader::new(match File::open(file) {
                    Ok(it) => it,
                    Err(err) => {
                        status_err!("{}", err);
                        RUSTIC_APP.shutdown(Shutdown::Crash);
                    }
                });
                match read_password_from_bufread(&mut file) {
                    Ok(it) => it,
                    Err(err) => {
                        status_err!("{}", err);
                        RUSTIC_APP.shutdown(Shutdown::Crash);
                    }
                }
            },
        );
        let ko = self.key_opts.clone();
        let keyfile = KeyFile::generate(key, &pass, ko.hostname, ko.username, ko.with_created)?;
        let data = serde_json::to_vec(&keyfile)?;
        let id = hash(&data);
        be.write_bytes(FileType::Key, &id, false, data.into())?;

        println!("key {id} successfully added.");

        Ok(())
    }
}
