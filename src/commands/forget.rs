//! `forget` subcommand

use crate::{
    commands::open_repository, helpers::table_with_titles, status_err, Application, RusticConfig,
    RUSTIC_APP,
};

use abscissa_core::{config::Override, Shutdown};
use abscissa_core::{Command, FrameworkError, Runnable};
use anyhow::Result;

use merge::Merge;
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};

use crate::{commands::prune::PruneCmd, filtering::SnapshotFilter};

use rustic_core::{
    ForgetGroup, ForgetGroups, ForgetSnapshot, KeepOptions, SnapshotGroup, SnapshotGroupCriterion,
};

/// `forget` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(super) struct ForgetCmd {
    /// Snapshots to forget. If none is given, use filter options to filter from all snapshots
    #[clap(value_name = "ID")]
    ids: Vec<String>,

    /// Show infos in json format
    #[clap(long)]
    json: bool,

    /// Don't show any output
    #[clap(long, conflicts_with = "json")]
    quiet: bool,

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
#[derive(Clone, Default, Debug, clap::Parser, Deserialize, Merge)]
#[serde(default, rename_all = "kebab-case")]
pub struct ForgetOptions {
    /// Group snapshots by any combination of host,label,paths,tags (default: "host,label,paths")
    #[clap(long, short = 'g', value_name = "CRITERION")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    group_by: Option<SnapshotGroupCriterion>,

    /// Also prune the repository
    #[clap(long)]
    #[merge(strategy = merge::bool::overwrite_false)]
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
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl ForgetCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();
        let repo = open_repository(&config.repository)?;

        let group_by = config.forget.group_by.unwrap_or_default();

        let groups = if self.ids.is_empty() {
            repo.get_forget_snapshots(&config.forget.keep, group_by, |sn| {
                config.forget.filter.matches(sn)
            })?
        } else {
            let item = ForgetGroup {
                group: SnapshotGroup::default(),
                snapshots: repo
                    .get_snapshots(&self.ids)?
                    .into_iter()
                    .map(|sn| ForgetSnapshot {
                        snapshot: sn,
                        keep: false,
                        reasons: vec!["id argument".to_string()],
                    })
                    .collect(),
            };
            ForgetGroups(vec![item])
        };

        if self.json {
            let mut stdout = std::io::stdout();
            serde_json::to_writer_pretty(&mut stdout, &groups)?;
        } else if !self.quiet {
            print_groups(&groups);
        }

        let forget_snaps = groups.into_forget_ids();

        match (forget_snaps.is_empty(), config.global.dry_run, self.json) {
            (true, _, false) => println!("nothing to remove"),
            (false, true, false) => {
                println!("would have removed the following snapshots:\n {forget_snaps:?}");
            }
            (false, false, _) => {
                repo.delete_snapshots(&forget_snaps)?;
            }
            (_, _, true) => {}
        }

        if self.config.prune {
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
    for ForgetGroup { group, snapshots } in &groups.0 {
        if !group.is_empty() {
            println!("snapshots for {group}");
        }
        let mut table = table_with_titles([
            "ID", "Time", "Host", "Label", "Tags", "Paths", "Action", "Reason",
        ]);

        for ForgetSnapshot {
            snapshot: sn,
            keep,
            reasons,
        } in snapshots
        {
            let time = sn.time.format("%Y-%m-%d %H:%M:%S").to_string();
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

        println!();
        println!("{table}");
        println!();
    }
}
