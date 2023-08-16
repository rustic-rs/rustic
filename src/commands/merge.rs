//! `merge` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{commands::open_repository, status_err, Application, RUSTIC_APP};
use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use log::info;

use chrono::Local;

use rustic_core::{Node, SnapshotFile, SnapshotOptions};

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

    #[clap(flatten, next_help_heading = "Snapshot options")]
    snap_opts: SnapshotOptions,
}

impl Runnable for MergeCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl MergeCmd {
    fn inner_run(&self) -> Result<()> {
        let now = Local::now();

        let command: String = std::env::args_os()
            .map(|s| s.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(" ");

        let config = RUSTIC_APP.config();
        let repo = open_repository(&config)?.to_indexed_ids()?;

        let snapshots = if self.ids.is_empty() {
            repo.get_matching_snapshots(|sn| config.snapshot_filter.matches(sn))?
        } else {
            repo.get_snapshots(&self.ids)?
        };

        let snap = SnapshotFile::new_from_options(&self.snap_opts, now, command)?;

        let cmp = |n1: &Node, n2: &Node| n1.meta.mtime.cmp(&n2.meta.mtime);

        let snap = repo.merge_snapshots(&snapshots, &cmp, snap)?;

        if self.json {
            let mut stdout = std::io::stdout();
            serde_json::to_writer_pretty(&mut stdout, &snap)?;
        }
        info!("saved new snapshot as {}.", snap.id);

        if self.delete {
            let now = Local::now();
            // TODO: Maybe use this check in repo.delete_snapshots?
            let snap_ids: Vec<_> = snapshots
                .iter()
                .filter(|sn| !sn.must_keep(now))
                .map(|sn| sn.id)
                .collect();
            repo.delete_snapshots(&snap_ids)?;
        }

        Ok(())
    }
}
