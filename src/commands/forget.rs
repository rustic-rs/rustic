//! `forget` subcommand

use crate::repository::{OpenRepo, get_grouped_snapshots};
use crate::{Application, RUSTIC_APP, RusticConfig, helpers::table_with_titles, status_err};

use abscissa_core::{Command, FrameworkError, Runnable};
use abscissa_core::{Shutdown, config::Override};
use anyhow::Result;
use conflate::Merge;
use jiff::Zoned;
use log::info;
use rustic_core::repofile::RusticTime;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};

use crate::{commands::prune::PruneCmd, filtering::SnapshotFilter};

use rustic_core::{ForgetGroups, ForgetSnapshot, KeepOptions, SnapshotGroupCriterion};

/// `forget` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(super) struct ForgetCmd {
    /// Snapshots to forget. If none is given, use filter options to filter from all snapshots
    ///
    /// Snapshots can be identified the following ways: "01a2b3c4" or "latest" or "latest~N" (N >= 0)
    #[clap(value_name = "ID")]
    ids: Vec<String>,

    /// Set the date/time (e.g. "2021-01-21) to use when evaluating retention rules; can be used to test the rules (default: now)
    #[clap(long,value_parser = RusticTime::parse_system)]
    pub forget_time: Option<Zoned>,

    /// Show infos in json format
    #[clap(long)]
    json: bool,

    /// Forget options
    #[clap(flatten)]
    config: ForgetOptions,

    /// Prune options (only when used with --prune)
    #[clap(
        flatten,
        next_help_heading = "PRUNE OPTIONS (only when used with --prune)"
    )]
    prune_opts: PruneCmd,
}

impl Override<RusticConfig> for ForgetCmd {
    // Process the given command line options, overriding settings from
    // a configuration file using explicit flags taken from command-line
    // arguments.
    fn override_config(&self, mut config: RusticConfig) -> Result<RusticConfig, FrameworkError> {
        let mut self_config = self.config.clone();
        // merge "forget" section from config file, if given
        self_config.merge(config.forget);
        // merge "snapshot-filter" section from config file, if given
        self_config.filter.merge(config.snapshot_filter.clone());
        config.forget = self_config;
        Ok(config)
    }
}

/// Forget options
#[serde_as]
#[derive(Clone, Default, Debug, clap::Parser, Serialize, Deserialize, Merge)]
#[serde(default, rename_all = "kebab-case")]
pub struct ForgetOptions {
    /// Group snapshots by any combination of host,label,paths,tags (default: "host,label,paths")
    #[clap(long, short = 'g', value_name = "CRITERION")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[merge(strategy=conflate::option::overwrite_none)]
    group_by: Option<SnapshotGroupCriterion>,

    /// Also prune the repository
    #[clap(long)]
    #[merge(strategy=conflate::bool::overwrite_false)]
    prune: bool,

    /// Snapshot filter options
    #[clap(flatten, next_help_heading = "Snapshot filter options")]
    #[serde(flatten)]
    filter: SnapshotFilter,

    /// Retention options
    #[clap(flatten, next_help_heading = "Retention options")]
    #[serde(flatten)]
    keep: KeepOptions,
}

impl Runnable for ForgetCmd {
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

impl ForgetCmd {
    /// be careful about self vs `RUSTIC_APP.config()` usage
    /// only the `RUSTIC_APP.config()` involves the TOML and ENV merged configurations
    /// see <https://github.com/rustic-rs/rustic/issues/1242>
    fn inner_run(&self, repo: OpenRepo) -> Result<()> {
        let config = RUSTIC_APP.config();

        let group_by = config
            .forget
            .group_by
            .or(config.global.group_by)
            .unwrap_or_default();

        if let Some(time) = &self.forget_time {
            info!("using time: {time}");
        }
        let now = self.forget_time.clone().unwrap_or_else(Zoned::now);

        let groups = if self.ids.is_empty() {
            ForgetGroups::from_grouped_snapshots_with_retention(
                get_grouped_snapshots(&repo, group_by, &[])?,
                &config.forget.keep,
                &now,
            )?
        } else {
            ForgetGroups::from_snapshots(
                repo.get_snapshots_from_strs(&self.ids, |sn| config.snapshot_filter.matches(sn))?,
                &now,
            )
        };

        if self.json {
            let mut stdout = std::io::stdout();
            serde_json::to_writer_pretty(&mut stdout, &groups)?;
        } else {
            print_groups(&groups);
        }

        let forget_snaps = groups.into_forget_ids();

        match (forget_snaps.is_empty(), config.global.dry_run, self.json) {
            (true, _, false) => info!("nothing to remove"),
            (false, true, false) => {
                info!("would have removed {} snapshots.", forget_snaps.len());
            }
            (false, false, _) => {
                repo.delete_snapshots(&forget_snaps)?;
            }
            (_, _, true) => {}
        }

        if config.forget.prune {
            let mut prune_opts = self.prune_opts.clone();
            prune_opts.opts.ignore_snaps = forget_snaps;
            prune_opts.run();
        }

        Ok(())
    }
}

/// Print groups to stdout
///
/// # Arguments
///
/// * `groups` - forget groups to print
fn print_groups(groups: &ForgetGroups) {
    let config = RUSTIC_APP.config();
    for group in &groups.0 {
        let mut table = table_with_titles([
            "ID", "Time", "Host", "Label", "Tags", "Paths", "Action", "Reason",
        ]);

        for ForgetSnapshot {
            snapshot: sn,
            keep,
            reasons,
        } in &group.items
        {
            let time = config.global.format_time(&sn.time).to_string();
            let tags = sn.tags.formatln();
            let paths = sn.paths.formatln();
            let action = if *keep { "keep" } else { "remove" };
            let reason = reasons.join("\n");
            _ = table.add_row([
                &sn.id.to_string(),
                &time,
                &sn.hostname,
                &sn.label,
                &tags,
                &paths,
                action,
                &reason,
            ]);
        }

        if !group.group_key.is_empty() {
            info!("snapshots for {}:\n{table}", group.group_key);
        } else {
            info!("snapshots:\n{table}");
        }
    }
}
