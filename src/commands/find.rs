//! `find` subcommand

use std::path::{Path, PathBuf};

use crate::{commands::open_repository_indexed, status_err, Application, RUSTIC_APP};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use globset::{Glob, GlobSetBuilder};
use itertools::Itertools;

use rustic_core::{repofile::Node, SnapshotGroupCriterion};

use super::ls::print_node;

/// `ls` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct FindCmd {
    /// pattern to find (can be specified multiple times)
    #[clap(long, value_name = "PATTERN")]
    patterns: Vec<String>,

    /// exact path to find
    #[clap(long, value_name = "PATH", conflicts_with = "patterns")]
    path: Option<PathBuf>,

    /// Snapshots to serach in. If none is given, use filter options to filter from all snapshots
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

    /// Also show snapshots which don't contain a search result.
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
            if let Some(path) = &self.path {
                let (nodes, results) = repo.find_nodes_from_path(ids, path)?;
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
                        print_node(&nodes[*idx], path, self.numeric_id);
                    }
                }
            } else {
                let mut builder = GlobSetBuilder::new();
                for pattern in &self.patterns {
                    _ = builder.add(Glob::new(pattern)?);
                }
                let globset = builder.build()?;
                let matches = |path: &Path, _: &Node| {
                    globset.is_match(path) || path.file_name().is_some_and(|f| globset.is_match(f))
                };
                let (paths, nodes, results) = repo.find_matching_nodes(ids, &matches)?;
                for (idx, mut g) in &results
                    .iter()
                    .zip(snapshots.iter())
                    .group_by(|(idx, _)| *idx)
                {
                    let not = if idx.is_empty() { "not " } else { "" };
                    if self.show_misses || !idx.is_empty() {
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
                    for (path_idx, node_idx) in idx {
                        print_node(&nodes[*node_idx], &paths[*path_idx], self.numeric_id);
                    }
                }
            }
        }
        Ok(())
    }
}
