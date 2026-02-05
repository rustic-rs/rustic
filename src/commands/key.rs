//! `key` subcommand

use crate::{
    Application, RUSTIC_APP, helpers::table_with_titles, repository::OpenRepo, status_err,
};

use std::path::PathBuf;

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::{Result, bail};
use dialoguer::Password;
use log::{info, warn};

use qrcode::{QrCode, render::svg};
use rustic_core::{
    CommandInput, CredentialOptions, Credentials, KeyOptions,
    repofile::{KeyFile, MasterKey},
};

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
    /// Export the masterkey
    Export(ExportCmd),
    /// Create a new masterkey
    Create(CreateCmd),
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
        // create new credential options which just contain password information
        let mut pass_opts = CredentialOptions::default();
        pass_opts.password = self.new_password.clone();
        pass_opts.password_file = self.new_password_file.clone();
        pass_opts.password_command = self.new_password_command.clone();

        let pass = if let Some(Credentials::Password(pass)) = pass_opts.credentials()? {
            pass
        } else {
            Password::new()
                .with_prompt(text)
                .allow_empty_password(true)
                .with_confirmation("confirm password", "passwords do not match")
                .interact()?
        };
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
    fn inner_run(&self, repo: OpenRepo) -> Result<()> {
        if RUSTIC_APP.config().global.dry_run {
            info!("adding no key in dry-run mode.");
            return Ok(());
        }
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
    fn inner_run(&self, repo: OpenRepo) -> Result<()> {
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
                format!(
                    "{}{}",
                    if used_key == &Some(key.0) { "*" } else { "" },
                    key.0
                ),
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
    /// The keys to remove
    ids: Vec<String>,
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
    fn inner_run(&self, repo: OpenRepo) -> Result<()> {
        let repo_key = repo.key_id();
        let ids: Vec<_> = repo.find_ids(&self.ids)?.collect();
        if ids.iter().any(|id| Some(id) == repo_key.as_ref()) {
            bail!("Cannot remove currently used key!");
        }
        if !RUSTIC_APP.config().global.dry_run {
            for id in ids {
                repo.delete_key(&id)?;
                info!("key {id} successfully removed.");
            }
            return Ok(());
        }

        let keys = repo
            .stream_files_list(&ids)?
            .inspect(|f| {
                if let Err(err) = f {
                    warn!("{err:?}");
                }
            })
            .filter_map(Result::ok);

        let mut table = table_with_titles(["ID", "User", "Host", "Created"]);
        _ = table.add_rows(keys.map(|key: (_, KeyFile)| {
            [
                key.0.to_string(),
                key.1.username.unwrap_or_default(),
                key.1.hostname.unwrap_or_default(),
                key.1
                    .created
                    .map_or(String::new(), |time| format!("{time}")),
            ]
        }));
        println!("would have removed the following keys:");
        println!("{table}");
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
    fn inner_run(&self, repo: OpenRepo) -> Result<()> {
        let Some(key_id) = repo.key_id() else {
            bail!("No keyfile used to open the repo. Cannot change the password.")
        };
        if RUSTIC_APP.config().global.dry_run {
            info!("changing no password in dry-run mode.");
            return Ok(());
        }
        let pass = self.pass_opts.pass("enter new password")?;
        let old_key: KeyFile = repo.get_file(key_id)?;
        let key_opts = KeyOptions::default()
            .hostname(old_key.hostname)
            .username(old_key.username)
            .with_created(old_key.created.is_some());
        let id = repo.add_key(&pass, &key_opts)?;
        info!("key {id} successfully added.");

        let old_key = *key_id; // copy key, as we need to use repo as reference
        // re-open repository using new password
        let repo = repo.open(&Credentials::Password(pass))?;
        repo.delete_key(&old_key)?;
        info!("key {old_key} successfully removed.");

        Ok(())
    }
}

#[derive(clap::Parser, Debug)]
pub(crate) struct ExportCmd {
    /// Write to file if given, else to stdout
    pub(crate) file: Option<PathBuf>,

    /// Generate a QR code in svg format
    #[clap(long)]
    pub(crate) qr: bool,
}

impl Runnable for ExportCmd {
    fn run(&self) {
        if let Err(err) = RUSTIC_APP.config().repository.run_open(|repo| {
            let mut data = serde_json::to_string(&repo.key())?;
            if self.qr {
                let qr = QrCode::new(&data)?;
                data = qr.render::<svg::Color<'_>>().build();
            }
            match &self.file {
                None => println!("{}", data),
                Some(file) => std::fs::write(file, data)?,
            }
            Ok(())
        }) {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

#[derive(clap::Parser, Debug)]
pub(crate) struct CreateCmd {
    /// Write to file if given, else to stdout
    pub(crate) file: Option<PathBuf>,
}

impl Runnable for CreateCmd {
    fn run(&self) {
        let inner = || -> Result<_> {
            let data = serde_json::to_string(&MasterKey::new())?;
            match &self.file {
                None => println!("{}", data),
                Some(file) => std::fs::write(file, data)?,
            }
            Ok(())
        };
        if let Err(err) = inner() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}
