//! `rewrite` subcommand

use crate::{
    Application, RUSTIC_APP,
    commands::snapshots::print_snapshots,
    repository::{CliIndexedRepo, CliOpenRepo, get_snapots_from_ids},
    status_err,
};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use log::info;

use rustic_core::{
    Excludes, NodeModification, RewriteOptions, RewriteTreesOptions, StringList,
    repofile::{SnapshotFile, SnapshotModification},
};

/// `rewrite` subcommand
#[derive(clap::Parser, Command, Debug, Default)]
pub(crate) struct RewriteCmd {
    /// Snapshots to rewrite. If none is given, use filter to filter from all snapshots.
    ///
    /// Snapshot can be identified the following ways: "01a2b3c4" or "latest" or "latest~N" (N >= 0)
    #[clap(value_name = "ID")]
    pub ids: Vec<String>,

    /// remove original snapshots
    #[clap(long)]
    pub forget: bool,

    /// Tags to add to rewritten snapshots [default: "rewrite" if original snapshots are not removed]
    #[clap(long, value_name = "TAG[,TAG,..]")]
    pub tags_rewritten: Option<StringList>,

    #[clap(flatten, next_help_heading = "Snapshot options")]
    pub modification: SnapshotModification,

    /// treat all trees as changed (i.e. serialize all and rebuild summary)
    #[clap(long, help_heading = "Tree rewrite options")]
    pub all_trees: bool,

    #[clap(flatten, next_help_heading = "Exclude options")]
    pub excludes: Excludes,

    #[clap(flatten, next_help_heading = "Node modification options")]
    pub node_modification: NodeModification,
}

impl Runnable for RewriteCmd {
    fn run(&self) {
        let repo = &RUSTIC_APP.config().repository;

        if let Err(err) =
            if self.excludes.is_empty() && self.node_modification.is_empty() && !self.all_trees {
                repo.run_open(|repo| self.inner_run_open(repo))
            } else {
                repo.run_indexed(|repo| self.inner_run_indexed(repo))
            }
        {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        }
    }
}

impl RewriteCmd {
    fn opts(&self) -> RewriteOptions {
        let config = RUSTIC_APP.config();
        RewriteOptions::default()
            .forget(self.forget)
            .tags_rewritten(self.tags_rewritten.clone())
            .modification(self.modification.clone())
            .dry_run(config.global.dry_run)
    }

    fn inner_run_open(&self, repo: CliOpenRepo) -> Result<()> {
        let snapshots = get_snapots_from_ids(&repo, &self.ids)?;

        let snaps = repo.rewrite_snapshots(snapshots, &self.opts())?;

        self.output(snaps);

        Ok(())
    }

    fn inner_run_indexed(&self, repo: CliIndexedRepo) -> Result<()> {
        let snapshots = get_snapots_from_ids(&repo, &self.ids)?;
        let tree_opts = RewriteTreesOptions::default()
            .all_trees(self.all_trees)
            .excludes(self.excludes.clone())
            .node_modification(self.node_modification.clone());

        let snaps = repo.rewrite_snapshots_and_trees(snapshots, &self.opts(), &tree_opts)?;

        self.output(snaps);

        Ok(())
    }

    fn output(&self, snaps: Vec<SnapshotFile>) {
        let config = RUSTIC_APP.config();
        if config.global.dry_run {
            println!("Would have rewritten the following snapshots:");
            print_snapshots(snaps, false, true);
        } else {
            info!("{} snapshots have been rewritten", snaps.len());
        }
    }
}
