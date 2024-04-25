//! `find` subcommand

use std::path::PathBuf;

use crate::{commands::open_repository_indexed, status_err, Application, RUSTIC_APP};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use itertools::Itertools;

use rustic_core::SnapshotGroupCriterion;

use super::ls::print_node;

/// `ls` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct FindCmd {
    /// Snapshot/path to list
    #[clap(value_name = "PATH")]
    path: PathBuf,

    /// Snapshots to show. If none is given, use filter options to filter from all snapshots
    #[clap(value_name = "ID")]
    ids: Vec<String>,

    /// Group snapshots by any combination of host,label,paths,tags
    #[clap(
        long,
        short = 'g',
        value_name = "CRITERION",
        default_value = "host,label,paths"
    )]
    group_by: SnapshotGroupCriterion,

    /// Show all snapshots instead of summarizing snapshots with identical search results
    #[clap(long)]
    all: bool,

    /// Also show snapshots which don't contain the searched path.
    #[clap(long)]
    show_misses: bool,

    /// Show long listing
    #[clap(long, short = 'l')]
    long: bool,

    /// Show uid/gid instead of user/group
    #[clap(long, long("numeric-uid-gid"))]
    numeric_id: bool,
}

impl Runnable for FindCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl FindCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();
        let repo = open_repository_indexed(&config.repository)?;

        let groups = repo.get_snapshot_group(&self.ids, self.group_by, |sn| {
            config.snapshot_filter.matches(sn)
        })?;
        for (group, mut snapshots) in groups {
            snapshots.sort_unstable();
            if !group.is_empty() {
                println!("\nsearching in snapshots group {group}");
            }
            let ids = snapshots.iter().map(|sn| sn.tree);
            let (nodes, results) = repo.find_nodes_from_path(ids, &self.path)?;
            for (idx, mut g) in &results
                .iter()
                .zip(snapshots.iter())
                .group_by(|(idx, _)| *idx)
            {
                let not = if idx.is_none() { "not " } else { "" };
                if self.show_misses || idx.is_some() {
                    if self.all {
                        for (_, sn) in g {
                            let time = sn.time.format("%Y-%m-%d %H:%M:%S");
                            println!("{not}found in {} from {time}", sn.id);
                        }
                    } else {
                        let (_, sn) = g.next().unwrap();
                        let count = g.count();
                        let time = sn.time.format("%Y-%m-%d %H:%M:%S");
                        match count {
                            0 => println!("{not}found in {} from {time}", sn.id),
                            count => println!("{not}found in {} from {time} (+{count})", sn.id),
                        };
                    }
                }
                if let Some(idx) = idx {
                    print_node(&nodes[*idx], &self.path, self.numeric_id);
                }
            }
        }
        Ok(())
    }
}
