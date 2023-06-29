//! `copy` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{
    commands::{get_repository, init::save_config, open_repository},
    helpers::copy,
    status_err, Application, RUSTIC_APP,
};
use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::{bail, Result};
use log::info;

use merge::Merge;
use serde::Deserialize;

use crate::commands::key::KeyOpts;
use rustic_core::{
    FileType, Id, IndexBackend, Open, ProgressBars, ReadBackend, Repository, RepositoryOptions,
    SnapshotFile,
};

/// `copy` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct CopyCmd {
    /// Snapshots to copy. If none is given, use filter options to filter from all snapshots.
    #[clap(value_name = "ID")]
    ids: Vec<String>,

    /// Initialize non-existing target repositories
    #[clap(long)]
    init: bool,

    #[clap(flatten, next_help_heading = "Key options (when using --init)")]
    key_opts: KeyOpts,
}

#[derive(Default, Clone, Debug, Deserialize, Merge)]
pub struct Targets {
    #[merge(strategy = merge::vec::overwrite_empty)]
    targets: Vec<RepositoryOptions>,
}

impl Runnable for CopyCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl CopyCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();

        let repo = open_repository(get_repository(&config));

        if config.copy.targets.is_empty() {
            status_err!("no [[copy.targets]] section in config file found!");
            RUSTIC_APP.shutdown(Shutdown::Crash);
        }

        let be = repo.dbe();
        let p = config.global.progress_options.progress_hidden();
        let mut snapshots = if self.ids.is_empty() {
            SnapshotFile::all_from_backend(be, |sn| config.snapshot_filter.matches(sn), &p)?
        } else {
            SnapshotFile::from_ids(be, &self.ids, &p)?
        };
        // sort for nicer output
        snapshots.sort_unstable();

        let index = IndexBackend::new(be, &config.global.progress_options.progress_counter(""))?;

        let poly = repo.config().poly()?;

        for target_opt in &config.copy.targets {
            let repo_dest = Repository::new(target_opt)?;

            if self.init && repo_dest.be.list(FileType::Config)?.is_empty() {
                let mut config_dest = repo.config().clone();
                config_dest.id = Id::random();
                save_config(
                    config_dest,
                    &repo_dest.be,
                    &repo_dest.be_hot,
                    self.key_opts.clone(),
                    repo_dest.password()?,
                )?;
            }

            let repo_dest = repo_dest.open()?;
            info!("copying to target {:?}...", repo_dest); // TODO: repo_dest.name
            if poly != repo_dest.config().poly()? {
                bail!("cannot copy to repository with different chunker parameter (re-chunking not implemented)!");
            }
            copy(&snapshots, &index, &repo_dest)?;
        }
        Ok(())
    }
}
