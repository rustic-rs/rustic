//! `self-update` subcommand

use crate::{Application, RUSTIC_APP};

use abscissa_core::{Command, Runnable, Shutdown, status_err};

use anyhow::Result;

/// `self-update` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct SelfUpdateCmd {
    /// Do not ask before processing the self-update
    #[clap(long, conflicts_with = "dry_run")]
    force: bool,
}

impl Runnable for SelfUpdateCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl SelfUpdateCmd {
    #[cfg(feature = "self-update")]
    fn inner_run(&self) -> Result<()> {
        let current_version = semver::Version::parse(self_update::cargo_crate_version!())?;

        let release = self_update::backends::github::Update::configure()
            .repo_owner("rustic-rs")
            .repo_name("rustic")
            .bin_name("rustic")
            .show_download_progress(true)
            .current_version(current_version.to_string().as_str())
            .no_confirm(self.force)
            .build()?;

        let latest_release = release.get_latest_release()?;

        let upstream_version = semver::Version::parse(&latest_release.version)?;

        match current_version.cmp(&upstream_version) {
            std::cmp::Ordering::Greater => {
                println!(
                    "Your rustic version {current_version} is newer than the stable version {upstream_version} on upstream!"
                );
            }
            std::cmp::Ordering::Equal => {
                println!("rustic version {current_version} is up-to-date!");
            }
            std::cmp::Ordering::Less => {
                let status = release.update()?;

                if let self_update::Status::Updated(str) = status {
                    println!("rustic version has been updated to: {str}");
                }
            }
        }

        Ok(())
    }
    #[cfg(not(feature = "self-update"))]
    fn inner_run(&self) -> Result<()> {
        anyhow::bail!(
            "This version of rustic was built without the \"self-update\" feature. Please use your system package manager to update it."
        );
    }
}
