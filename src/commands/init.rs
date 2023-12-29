//! `init` subcommand

use abscissa_core::{status_err, Command, Runnable, Shutdown};
use anyhow::{bail, Result};

use crate::{Application, RUSTIC_APP};

use dialoguer::Password;

use rustic_core::{ConfigOptions, KeyOptions, OpenStatus, Repository};

/// `init` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct InitCmd {
    /// Key options
    #[clap(flatten, next_help_heading = "Key options")]
    key_opts: KeyOptions,

    /// Config options
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
        let backends = config.backend.to_backends()?;

        let po = config.global.progress_options;
        let repo = Repository::new_with_progress(&config.repository, backends, po)?;

        // Note: This is again checked in repo.init_with_password(), however we want to inform
        // users before they are prompted to enter a password
        if repo.config_id()?.is_some() {
            bail!("Config file already exists. Aborting.");
        }

        // Handle dry-run mode
        if config.global.dry_run {
            bail!(
                "cannot initialize repository {} in dry-run mode!",
                repo.name
            );
        }

        let _ = init(repo, &self.key_opts, &self.config_opts)?;
        Ok(())
    }
}

/// Initialize repository
///
/// # Arguments
///
/// * `repo` - Repository to initialize
/// * `key_opts` - Key options
/// * `config_opts` - Config options
///
/// # Errors
///
///  * [`RepositoryErrorKind::OpeningPasswordFileFailed`] - If opening the password file failed
/// * [`RepositoryErrorKind::ReadingPasswordFromReaderFailed`] - If reading the password failed
/// * [`RepositoryErrorKind::FromSplitError`] - If splitting the password command failed
/// * [`RepositoryErrorKind::PasswordCommandParsingFailed`] - If parsing the password command failed
/// * [`RepositoryErrorKind::ReadingPasswordFromCommandFailed`] - If reading the password from the command failed
///
/// # Returns
///
/// Returns the initialized repository
///
/// [`RepositoryErrorKind::OpeningPasswordFileFailed`]: rustic_core::error::RepositoryErrorKind::OpeningPasswordFileFailed
/// [`RepositoryErrorKind::ReadingPasswordFromReaderFailed`]: rustic_core::error::RepositoryErrorKind::ReadingPasswordFromReaderFailed
/// [`RepositoryErrorKind::FromSplitError`]: rustic_core::error::RepositoryErrorKind::FromSplitError
/// [`RepositoryErrorKind::PasswordCommandParsingFailed`]: rustic_core::error::RepositoryErrorKind::PasswordCommandParsingFailed
/// [`RepositoryErrorKind::ReadingPasswordFromCommandFailed`]: rustic_core::error::RepositoryErrorKind::ReadingPasswordFromCommandFailed
pub(crate) fn init<P, S>(
    repo: Repository<P, S>,
    key_opts: &KeyOptions,
    config_opts: &ConfigOptions,
) -> Result<Repository<P, OpenStatus>> {
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

    Ok(repo.init_with_password(&pass, key_opts, config_opts)?)
}
