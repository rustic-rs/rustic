//! `self-update` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{Application, RUSTIC_APP};

use abscissa_core::{status_err, Command, Runnable, Shutdown};

use anyhow::Result;
use self_update::cargo_crate_version;
use semver::Version;

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
    fn inner_run(&self) -> Result<()> {
        let current_version = Version::parse(cargo_crate_version!())?;

        let release = self_update::backends::github::Update::configure()
            .repo_owner("rustic-rs")
            .repo_name("rustic")
            .bin_name("rustic")
            .show_download_progress(true)
            .current_version(current_version.to_string().as_str())
            .no_confirm(self.force)
            .build()?;

        let latest_release = release.get_latest_release()?;

        let upstream_version = Version::parse(&latest_release.version)?;

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
}
