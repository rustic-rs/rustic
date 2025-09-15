//! `copy` subcommand

use crate::{
    Application, RUSTIC_APP, RusticConfig,
    commands::init::init_password,
    helpers::table_with_titles,
    repository::{CliIndexedRepo, CliRepo},
    status_err,
};
use abscissa_core::{Command, FrameworkError, Runnable, Shutdown, config::Override};
use anyhow::{Result, bail};
use conflate::Merge;
use log::{Level, error, info, log};
use serde::{Deserialize, Serialize};

use rustic_core::{CopySnapshot, Id, KeyOptions, repofile::SnapshotFile};

/// `copy` subcommand
#[derive(clap::Parser, Command, Default, Clone, Debug, Serialize, Deserialize, Merge)]
pub struct CopyCmd {
    /// Snapshots to copy. If none is given, use filter options to filter from all snapshots.
    /// Snapshots can be identified the following ways: "01a2b3c4" or "latest" or "latest~N" (N >= 0)
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
    #[clap(long = "target", value_name = "TARGET")]
    #[merge(strategy=conflate::vec::overwrite_empty)]
    targets: Vec<String>,

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
        // merge "copy" section from config file, if given
        self_config.merge(config.copy);
        config.copy = self_config;
        Ok(config)
    }
}

impl Runnable for CopyCmd {
    fn run(&self) {
        let config = RUSTIC_APP.config();
        if config.copy.targets.is_empty() {
            status_err!(
                "No target given. Please specify at least 1 target either in the profile or using --target!"
            );
            RUSTIC_APP.shutdown(Shutdown::Crash);
        }
        if let Err(err) = config.repository.run_indexed(|repo| self.inner_run(repo)) {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl CopyCmd {
    fn inner_run(&self, repo: CliIndexedRepo) -> Result<()> {
        let config = RUSTIC_APP.config();
        let mut snapshots = if self.ids.is_empty() {
            repo.get_matching_snapshots(|sn| config.snapshot_filter.matches(sn))?
        } else {
            repo.get_snapshots_from_strs(&self.ids, |sn| config.snapshot_filter.matches(sn))?
        };
        // sort for nicer output
        snapshots.sort_unstable();

        for target in &config.copy.targets {
            let mut merge_logs = Vec::new();
            let mut target_config = RusticConfig::default();
            target_config.merge_profile(target, &mut merge_logs, Level::Error)?;
            // display logs from merging
            for (level, merge_log) in merge_logs {
                log!(level, "{merge_log}");
            }
            let target_opt = &target_config.repository;
            if let Err(err) =
                target_opt.run(|target_repo| self.copy(&repo, target_repo, &snapshots))
            {
                error!("error copying to target: {err}");
            }
        }
        Ok(())
    }

    fn copy(
        &self,
        repo: &CliIndexedRepo,
        target_repo: CliRepo,
        snapshots: &[SnapshotFile],
    ) -> Result<()> {
        let config = RUSTIC_APP.config();

        info!("copying to target {}...", target_repo.name);
        let target_repo = if self.init && target_repo.config_id()?.is_none() {
            let mut config_dest = repo.config().clone();
            config_dest.id = Id::random().into();
            let pass = init_password(&target_repo)?;
            target_repo
                .0
                .init_with_config(&pass, &self.key_opts, config_dest)?
        } else {
            target_repo.open()?
        };

        if !repo.config().has_same_chunker(target_repo.config()) {
            bail!(
                "cannot copy to repository with different chunker parameter (re-chunking not implemented)!"
            );
        }

        let snaps = target_repo.relevant_copy_snapshots(
            |sn| !self.ids.is_empty() || config.snapshot_filter.matches(sn),
            snapshots,
        )?;

        let mut table =
            table_with_titles(["ID", "Time", "Host", "Label", "Tags", "Paths", "Status"]);
        for CopySnapshot { relevant, sn } in &snaps {
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
                    &target_repo.to_indexed_ids()?,
                    snaps
                        .iter()
                        .filter_map(|CopySnapshot { relevant, sn }| relevant.then_some(sn)),
                )?;
            }
        } else {
            info!("nothing to copy.");
        }
        Ok(())
    }
}
