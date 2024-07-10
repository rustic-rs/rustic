//! `copy` subcommand

use crate::{
    commands::{get_repository, init::init_password, open_repository, open_repository_indexed},
    helpers::table_with_titles,
    status_err, Application, RusticConfig, RUSTIC_APP,
};
use abscissa_core::{config::Override, Command, FrameworkError, Runnable, Shutdown};
use anyhow::{bail, Result};
use log::{error, info, log, Level};
use merge::Merge;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, OneOrMany};

use rustic_core::{CopySnapshot, Id, KeyOptions};

/// `copy` subcommand
#[serde_as]
#[derive(clap::Parser, Command, Default, Clone, Debug, Serialize, Deserialize, Merge)]
pub struct CopyCmd {
    /// Snapshots to copy. If none is given, use filter options to filter from all snapshots.
    #[clap(value_name = "ID")]
    #[serde(skip)]
    #[merge(skip)]
    ids: Vec<String>,

    /// Initialize non-existing target repositories
    #[clap(long)]
    #[serde(skip)]
    #[merge(skip)]
    init: bool,

    /// Target repository (can be specified multiple times)
    #[clap(long)]
    #[merge(strategy = merge::vec::overwrite_empty)]
    #[serde_as(as = "OneOrMany<_>")]
    target: Vec<String>,

    /// Key options (when using --init)
    #[clap(flatten, next_help_heading = "Key options (when using --init)")]
    #[serde(skip)]
    #[merge(skip)]
    key_opts: KeyOptions,
}

impl Override<RusticConfig> for CopyCmd {
    // Process the given command line options, overriding settings from
    // a configuration file using explicit flags taken from command-line
    // arguments.
    fn override_config(&self, mut config: RusticConfig) -> Result<RusticConfig, FrameworkError> {
        let mut self_config = self.clone();
        // merge "webdav" section from config file, if given
        self_config.merge(config.copy);
        config.copy = self_config;
        Ok(config)
    }
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

        if config.copy.target.is_empty() {
            status_err!("please specify at least 1 target!");
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
        for target in &config.copy.target {
            let mut merge_logs = Vec::new();
            let mut target_config = RusticConfig::default();
            target_config.merge_profile(target, &mut merge_logs, Level::Warn)?;
            // display logs from merging
            for (level, merge_log) in merge_logs {
                log!(level, "{}", merge_log);
            }
            let target_opt = &target_config.repository;

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
