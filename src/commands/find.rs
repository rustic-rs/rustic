//! `find` subcommand

use std::path::{Path, PathBuf};

use crate::{Application, RUSTIC_APP, repository::CliIndexedRepo, status_err};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use clap::ValueHint;
use globset::{Glob, GlobBuilder, GlobSetBuilder};
use itertools::Itertools;

use rustic_core::{
    FindMatches, FindNode, SnapshotGroupCriterion,
    repofile::{Node, SnapshotFile},
};

use super::ls::print_node;

/// `find` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct FindCmd {
    /// pattern to find (can be specified multiple times)
    #[clap(long, value_name = "PATTERN", conflicts_with = "path")]
    glob: Vec<String>,

    /// pattern to find case-insensitive (can be specified multiple times)
    #[clap(long, value_name = "PATTERN", conflicts_with = "path")]
    iglob: Vec<String>,

    /// exact path to find
    #[clap(long, value_name = "PATH", value_hint = ValueHint::AnyPath)]
    path: Option<PathBuf>,

    /// Snapshots to search in. If none is given, use filter options to filter from all snapshots
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

    /// Show uid/gid instead of user/group
    #[clap(long, long("numeric-uid-gid"))]
    numeric_id: bool,
}

impl Runnable for FindCmd {
    fn run(&self) {
        if let Err(err) = RUSTIC_APP
            .config()
            .repository
            .run_indexed(|repo| self.inner_run(repo))
        {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl FindCmd {
    fn inner_run(&self, repo: CliIndexedRepo) -> Result<()> {
        let config = RUSTIC_APP.config();

        let groups = repo.get_snapshot_group(&self.ids, self.group_by, |sn| {
            config.snapshot_filter.matches(sn)
        })?;
        for (group, mut snapshots) in groups {
            snapshots.sort_unstable();
            if !group.is_empty() {
                println!("\nsearching in snapshots group {group}...");
            }
            let ids = snapshots.iter().map(|sn| sn.tree);
            if let Some(path) = &self.path {
                let FindNode { nodes, matches } = repo.find_nodes_from_path(ids, path)?;
                for (idx, g) in &matches
                    .iter()
                    .zip(snapshots.iter())
                    .chunk_by(|(idx, _)| *idx)
                {
                    self.print_identical_snapshots(idx.iter(), g.into_iter().map(|(_, sn)| sn));
                    if let Some(idx) = idx {
                        print_node(&nodes[*idx], path, self.numeric_id);
                    }
                }
            } else {
                let mut builder = GlobSetBuilder::new();
                for glob in &self.glob {
                    _ = builder.add(Glob::new(glob)?);
                }
                for glob in &self.iglob {
                    _ = builder.add(GlobBuilder::new(glob).case_insensitive(true).build()?);
                }
                let globset = builder.build()?;
                let matches = |path: &Path, _: &Node| {
                    globset.is_match(path) || path.file_name().is_some_and(|f| globset.is_match(f))
                };
                let FindMatches {
                    paths,
                    nodes,
                    matches,
                } = repo.find_matching_nodes(ids, &matches)?;
                for (idx, g) in &matches
                    .iter()
                    .zip(snapshots.iter())
                    .chunk_by(|(idx, _)| *idx)
                {
                    self.print_identical_snapshots(idx.iter(), g.into_iter().map(|(_, sn)| sn));
                    for (path_idx, node_idx) in idx {
                        print_node(&nodes[*node_idx], &paths[*path_idx], self.numeric_id);
                    }
                }
            }
        }
        Ok(())
    }

    fn print_identical_snapshots<'a>(
        &self,
        mut idx: impl Iterator,
        mut g: impl Iterator<Item = &'a SnapshotFile>,
    ) {
        let empty_result = idx.next().is_none();
        let not = if empty_result { "not " } else { "" };
        if self.show_misses || !empty_result {
            if self.all {
                for sn in g {
                    let time = sn.time.format("%Y-%m-%d %H:%M:%S");
                    println!("{not}found in {} from {time}", sn.id);
                }
            } else {
                let sn = g.next().unwrap();
                let count = g.count();
                let time = sn.time.format("%Y-%m-%d %H:%M:%S");
                match count {
                    0 => println!("{not}found in {} from {time}", sn.id),
                    count => println!("{not}found in {} from {time} (+{count})", sn.id),
                };
            }
        }
    }
}
