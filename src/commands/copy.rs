//! `copy` subcommand

use crate::{
    commands::{get_repository, init::init_password, open_repository, open_repository_indexed},
    config::AllRepositoryOptions,
    helpers::table_with_titles,
    status_err, Application, RUSTIC_APP,
};
use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::{bail, Result};
use log::{error, info};
use merge::Merge;
use serde::Deserialize;

use rustic_core::{CopySnapshot, Id, KeyOptions};

/// `copy` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct CopyCmd {
    /// Snapshots to copy. If none is given, use filter options to filter from all snapshots.
    #[clap(value_name = "ID")]
    ids: Vec<String>,

    /// Initialize non-existing target repositories
    #[clap(long)]
    init: bool,

    /// Key options (when using --init)
    #[clap(flatten, next_help_heading = "Key options (when using --init)")]
    key_opts: KeyOptions,
}

/// Target repository options
#[derive(Default, Clone, Debug, Deserialize, Merge)]
pub struct Targets {
    /// Target repositories
    #[merge(strategy = merge::vec::overwrite_empty)]
    targets: Vec<AllRepositoryOptions>,
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

        if config.copy.targets.is_empty() {
            status_err!("no [[copy.targets]] section in config file found!");
            RUSTIC_APP.shutdown(Shutdown::Crash);
        }

        let repo = open_repository_indexed(&config.repository)?;
        let mut snapshots = if self.ids.is_empty() {
            repo.get_matching_snapshots(|sn| config.snapshot_filter.matches(sn))?
        } else {
            repo.get_snapshots(&self.ids)?
        };
        // sort for nicer output
        snapshots.sort_unstable();

        let poly = repo.config().poly()?;
        for target_opt in &config.copy.targets {
            let repo_dest = get_repository(target_opt)?;

            info!("copying to target {}...", repo_dest.name);
            let repo_dest = if self.init && repo_dest.config_id()?.is_none() {
                if config.global.dry_run {
                    error!(
                        "cannot initialize target {} in dry-run mode!",
                        repo_dest.name
                    );
                    continue;
                }
                let mut config_dest = repo.config().clone();
                config_dest.id = Id::random();
                let pass = init_password(&repo_dest)?;
                repo_dest.init_with_config(&pass, &self.key_opts, config_dest)?
            } else {
                open_repository(target_opt)?
            };

            if poly != repo_dest.config().poly()? {
                bail!("cannot copy to repository with different chunker parameter (re-chunking not implemented)!");
            }

            let snaps = repo_dest.relevant_copy_snapshots(
                |sn| !self.ids.is_empty() || config.snapshot_filter.matches(sn),
                &snapshots,
            )?;

            let mut table =
                table_with_titles(["ID", "Time", "Host", "Label", "Tags", "Paths", "Status"]);
            for CopySnapshot { relevant, sn } in snaps.iter() {
                let tags = sn.tags.formatln();
                let paths = sn.paths.formatln();
                let time = sn.time.format("%Y-%m-%d %H:%M:%S").to_string();
                _ = table.add_row([
                    &sn.id.to_string(),
                    &time,
                    &sn.hostname,
                    &sn.label,
                    &tags,
                    &paths,
                    &(if *relevant { "to copy" } else { "existing" }).to_string(),
                ]);
            }
            println!("{table}");

            let count = snaps.iter().filter(|sn| sn.relevant).count();
            if count > 0 {
                if config.global.dry_run {
                    info!("would have copied {count} snapshots.");
                } else {
                    repo.copy(
                        &repo_dest.to_indexed_ids()?,
                        snaps
                            .iter()
                            .filter_map(|CopySnapshot { relevant, sn }| relevant.then_some(sn)),
                    )?;
                }
            } else {
                info!("nothing to copy.");
            }
        }
        Ok(())
    }
}
