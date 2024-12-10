//! `key` subcommand

use crate::{
    helpers::table_with_titles, repository::CliOpenRepo, status_err, Application, RUSTIC_APP,
};

use std::path::PathBuf;

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use dialoguer::Password;
use log::{info, warn};

use rustic_core::{repofile::KeyFile, CommandInput, KeyOptions, RepositoryOptions};

/// `key` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(super) struct KeyCmd {
    /// Subcommand to run
    #[clap(subcommand)]
    cmd: KeySubCmd,
}

impl Runnable for KeyCmd {
    fn run(&self) {
        self.cmd.run();
    }
}

#[derive(clap::Subcommand, Debug, Runnable)]
enum KeySubCmd {
    /// Add a new key to the repository
    Add(AddCmd),
    /// List all keys in the repository
    List(ListCmd),
    /// Remove a key from the repository
    Remove(RemoveCmd),
    /// Change the password of a key
    Password(PasswordCmd),
}

#[derive(clap::Parser, Debug)]
pub(crate) struct NewPasswordOptions {
    /// New password
    #[clap(long)]
    pub(crate) new_password: Option<String>,

    /// File from which to read the new password
    #[clap(long)]
    pub(crate) new_password_file: Option<PathBuf>,

    /// Command to get the new password from
    #[clap(long)]
    pub(crate) new_password_command: Option<CommandInput>,
}

impl NewPasswordOptions {
    fn pass(&self, text: &str) -> Result<String> {
        // create new Repository options which just contain password information
        let mut pass_opts = RepositoryOptions::default();
        pass_opts.password = self.new_password.clone();
        pass_opts.password_file = self.new_password_file.clone();
        pass_opts.password_command = self.new_password_command.clone();

        let pass = pass_opts
            .evaluate_password()
            .map_err(Into::into)
            .transpose()
            .unwrap_or_else(|| -> Result<_> {
                Ok(Password::new()
                    .with_prompt(text)
                    .allow_empty_password(true)
                    .with_confirmation("confirm password", "passwords do not match")
                    .interact()?)
            })?;
        Ok(pass)
    }
}

#[derive(clap::Parser, Debug)]
pub(crate) struct AddCmd {
    /// New password options
    #[clap(flatten)]
    pub(crate) pass_opts: NewPasswordOptions,

    /// Key options
    #[clap(flatten)]
    pub(crate) key_opts: KeyOptions,
}

impl Runnable for AddCmd {
    fn run(&self) {
        if let Err(err) = RUSTIC_APP
            .config()
            .repository
            .run_open(|repo| self.inner_run(repo))
        {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl AddCmd {
    fn inner_run(&self, repo: CliOpenRepo) -> Result<()> {
        let pass = self.pass_opts.pass("enter password for new key")?;
        let id = repo.add_key(&pass, &self.key_opts)?;
        info!("key {id} successfully added.");

        Ok(())
    }
}

#[derive(clap::Parser, Debug)]
pub(crate) struct ListCmd;

impl Runnable for ListCmd {
    fn run(&self) {
        if let Err(err) = RUSTIC_APP
            .config()
            .repository
            .run_open(|repo| self.inner_run(repo))
        {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl ListCmd {
    fn inner_run(&self, repo: CliOpenRepo) -> Result<()> {
        let used_key = repo.key_id();
        let keys = repo
            .stream_files()?
            .inspect(|f| {
                if let Err(err) = f {
                    warn!("{err:?}");
                }
            })
            .filter_map(Result::ok);

        let mut table = table_with_titles(["ID", "User", "Host", "Created"]);
        _ = table.add_rows(keys.map(|key: (_, KeyFile)| {
            [
                format!("{}{}", if used_key == &key.0 { "*" } else { "" }, key.0),
                key.1.username.unwrap_or_default(),
                key.1.hostname.unwrap_or_default(),
                key.1
                    .created
                    .map_or(String::new(), |time| format!("{time}")),
            ]
        }));
        println!("{table}");
        Ok(())
    }
}

#[derive(clap::Parser, Debug)]
pub(crate) struct RemoveCmd {
    /// The key is to remove
    id: String,
}

impl Runnable for RemoveCmd {
    fn run(&self) {
        if let Err(err) = RUSTIC_APP
            .config()
            .repository
            .run_open(|repo| self.inner_run(repo))
        {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl RemoveCmd {
    fn inner_run(&self, repo: CliOpenRepo) -> Result<()> {
        repo.delete_key(&self.id)?;
        info!("key {} successfully removed.", self.id);
        Ok(())
    }
}
#[derive(clap::Parser, Debug)]
pub(crate) struct PasswordCmd {
    /// New password options
    #[clap(flatten)]
    pub(crate) pass_opts: NewPasswordOptions,
}

impl Runnable for PasswordCmd {
    fn run(&self) {
        if let Err(err) = RUSTIC_APP
            .config()
            .repository
            .run_open(|repo| self.inner_run(repo))
        {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl PasswordCmd {
    fn inner_run(&self, repo: CliOpenRepo) -> Result<()> {
        let pass = self.pass_opts.pass("enter new password")?;
        let old_key: KeyFile = repo.get_file(repo.key_id())?;
        let key_opts = KeyOptions::default()
            .hostname(old_key.hostname)
            .username(old_key.username)
            .with_created(old_key.created.is_some());
        let id = repo.add_key(&pass, &key_opts)?;
        info!("key {id} successfully added.");

        let old_key = *repo.key_id();
        // re-open repository using new password
        let repo = repo.open_with_password(&pass)?;
        repo.delete_key(&old_key.to_string())?;
        info!("key {old_key} successfully removed.");

        Ok(())
    }
}
