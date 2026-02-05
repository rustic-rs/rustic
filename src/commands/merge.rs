//! `merge` subcommand

use crate::{
    Application, RUSTIC_APP,
    repository::{OpenRepo, get_snapots_from_ids},
    status_err,
};
use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use jiff::Zoned;
use log::info;

use rustic_core::{SnapshotOptions, last_modified_node, repofile::SnapshotFile};

/// `merge` subcommand
#[derive(clap::Parser, Default, Command, Debug)]
pub(super) struct MergeCmd {
    /// Snapshots to merge. If none is given, use filter options to filter from all snapshots.
    #[clap(value_name = "ID")]
    ids: Vec<String>,

    /// Output generated snapshot in json format
    #[clap(long)]
    json: bool,

    /// Remove input snapshots after merging
    #[clap(long)]
    delete: bool,

    /// Snapshot options
    #[clap(flatten, next_help_heading = "Snapshot options")]
    snap_opts: SnapshotOptions,
}

impl Runnable for MergeCmd {
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

impl MergeCmd {
    fn inner_run(&self, repo: OpenRepo) -> Result<()> {
        let config = RUSTIC_APP.config();
        let repo = repo.to_indexed_ids()?;

        let snapshots = get_snapots_from_ids(&repo, &self.ids)?;

        // Handle dry-run mode
        if config.global.dry_run {
            println!("would have modified the following snapshots:\n {snapshots:?}");
            return Ok(());
        }

        let snap = SnapshotFile::from_options(&self.snap_opts)?;
        let snap = repo.merge_snapshots(&snapshots, &last_modified_node, snap)?;

        if self.json {
            let mut stdout = std::io::stdout();
            serde_json::to_writer_pretty(&mut stdout, &snap)?;
        }
        info!("saved new snapshot as {}.", snap.id);

        if self.delete {
            let now = Zoned::now();
            // TODO: Maybe use this check in repo.delete_snapshots?
            let snap_ids: Vec<_> = snapshots
                .iter()
                .filter(|sn| !sn.must_keep(&now))
                .map(|sn| sn.id)
                .collect();
            repo.delete_snapshots(&snap_ids)?;
        }

        Ok(())
    }
}
