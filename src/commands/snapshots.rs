//! `smapshot` subcommand

use crate::{
    helpers::{bold_cell, bytes_size_to_string, table, table_right_from},
    repository::CliOpenRepo,
    status_err, Application, RUSTIC_APP,
};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use comfy_table::Cell;
use derive_more::From;
use humantime::format_duration;
use itertools::Itertools;
use serde::Serialize;

use rustic_core::{
    repofile::{DeleteOption, SnapshotFile},
    SnapshotGroup, SnapshotGroupCriterion,
};

#[cfg(feature = "tui")]
use super::tui;

/// `snapshot` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct SnapshotCmd {
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

    /// Show detailed information about snapshots
    #[arg(long)]
    long: bool,

    /// Show snapshots in json format
    #[clap(long, conflicts_with = "long")]
    json: bool,

    /// Show all snapshots instead of summarizing identical follow-up snapshots
    #[clap(long, conflicts_with_all = &["long", "json"])]
    all: bool,

    #[cfg(feature = "tui")]
    /// Run in interactive UI mode
    #[clap(long, short)]
    pub interactive: bool,
}

impl Runnable for SnapshotCmd {
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

impl SnapshotCmd {
    fn inner_run(&self, repo: CliOpenRepo) -> Result<()> {
        #[cfg(feature = "tui")]
        if self.interactive {
            return tui::run(self.group_by);
        }

        let config = RUSTIC_APP.config();

        let groups = repo.get_snapshot_group(&self.ids, self.group_by, |sn| {
            config.snapshot_filter.matches(sn)
        })?;

        if self.json {
            #[derive(Serialize, From)]
            struct SnapshotsGroup {
                group_key: SnapshotGroup,
                snapshots: Vec<SnapshotFile>,
            }
            let groups: Vec<SnapshotsGroup> = groups.into_iter().map(|g| g.into()).collect();
            let mut stdout = std::io::stdout();
            if groups.len() == 1 && groups[0].group_key.is_empty() {
                // we don't use grouping, only output snapshots list
                serde_json::to_writer_pretty(&mut stdout, &groups[0].snapshots)?;
            } else {
                serde_json::to_writer_pretty(&mut stdout, &groups)?;
            }
            return Ok(());
        }

        let mut total_count = 0;
        for (group_key, mut snapshots) in groups {
            if !group_key.is_empty() {
                println!("\nsnapshots for {group_key}");
            }
            snapshots.sort_unstable();
            let count = snapshots.len();

            if self.long {
                for snap in snapshots {
                    let mut table = table();

                    let add_entry = |title: &str, value: String| {
                        _ = table.add_row([bold_cell(title), Cell::new(value)]);
                    };
                    fill_table(&snap, add_entry);

                    println!("{table}");
                    println!();
                }
            } else {
                let mut table = table_right_from(
                    6,
                    [
                        "ID", "Time", "Host", "Label", "Tags", "Paths", "Files", "Dirs", "Size",
                    ],
                );

                if self.all {
                    // Add all snapshots to output table
                    _ = table.add_rows(snapshots.into_iter().map(|sn| snap_to_table(&sn, 0)));
                } else {
                    // Group snapshts by treeid and output into table
                    _ = table.add_rows(
                        snapshots
                            .into_iter()
                            .chunk_by(|sn| sn.tree)
                            .into_iter()
                            .map(|(_, mut g)| snap_to_table(&g.next().unwrap(), g.count())),
                    );
                }
                println!("{table}");
            }
            println!("{count} snapshot(s)");
            total_count += count;
        }
        println!();
        println!("total: {total_count} snapshot(s)");

        Ok(())
    }
}

pub fn snap_to_table(sn: &SnapshotFile, count: usize) -> [String; 9] {
    let tags = sn.tags.formatln();
    let paths = sn.paths.formatln();
    let time = sn.time.format("%Y-%m-%d %H:%M:%S");
    let (files, dirs, size) = sn.summary.as_ref().map_or_else(
        || ("?".to_string(), "?".to_string(), "?".to_string()),
        |s| {
            (
                s.total_files_processed.to_string(),
                s.total_dirs_processed.to_string(),
                bytes_size_to_string(s.total_bytes_processed),
            )
        },
    );
    let id = match count {
        0 => format!("{}", sn.id),
        count => format!("{} (+{})", sn.id, count),
    };
    [
        id,
        time.to_string(),
        sn.hostname.clone(),
        sn.label.clone(),
        tags,
        paths,
        files,
        dirs,
        size,
    ]
}

pub fn fill_table(snap: &SnapshotFile, mut add_entry: impl FnMut(&str, String)) {
    add_entry("Snapshot", snap.id.to_hex().to_string());
    // note that if original was not set, it is set to snap.id by the load process
    if let Some(original) = snap.original {
        if original != snap.id {
            add_entry("Original ID", original.to_hex().to_string());
        }
    }
    add_entry("Time", snap.time.format("%Y-%m-%d %H:%M:%S").to_string());
    add_entry("Generated by", snap.program_version.clone());
    add_entry("Host", snap.hostname.clone());
    add_entry("Label", snap.label.clone());
    add_entry("Tags", snap.tags.formatln());
    let delete = match snap.delete {
        DeleteOption::NotSet => "not set".to_string(),
        DeleteOption::Never => "never".to_string(),
        DeleteOption::After(t) => format!("after {}", t.format("%Y-%m-%d %H:%M:%S")),
    };
    add_entry("Delete", delete);
    add_entry("Paths", snap.paths.formatln());
    let parent = snap.parent.map_or_else(
        || "no parent snapshot".to_string(),
        |p| p.to_hex().to_string(),
    );
    add_entry("Parent", parent);
    if let Some(ref summary) = snap.summary {
        add_entry("", String::new());
        add_entry("Command", summary.command.clone());

        let source = format!(
            "files: {} / dirs: {} / size: {}",
            summary.total_files_processed,
            summary.total_dirs_processed,
            bytes_size_to_string(summary.total_bytes_processed)
        );
        add_entry("Source", source);
        add_entry("", String::new());

        let files = format!(
            "new: {:>10} / changed: {:>10} / unchanged: {:>10}",
            summary.files_new, summary.files_changed, summary.files_unmodified,
        );
        add_entry("Files", files);

        let trees = format!(
            "new: {:>10} / changed: {:>10} / unchanged: {:>10}",
            summary.dirs_new, summary.dirs_changed, summary.dirs_unmodified,
        );
        add_entry("Dirs", trees);
        add_entry("", String::new());

        let written = format!(
            "data:  {:>10} blobs / raw: {:>10} / packed: {:>10}\n\
            tree:  {:>10} blobs / raw: {:>10} / packed: {:>10}\n\
            total: {:>10} blobs / raw: {:>10} / packed: {:>10}",
            summary.data_blobs,
            bytes_size_to_string(summary.data_added_files),
            bytes_size_to_string(summary.data_added_files_packed),
            summary.tree_blobs,
            bytes_size_to_string(summary.data_added_trees),
            bytes_size_to_string(summary.data_added_trees_packed),
            summary.tree_blobs + summary.data_blobs,
            bytes_size_to_string(summary.data_added),
            bytes_size_to_string(summary.data_added_packed),
        );
        add_entry("Added to repo", written);

        let duration = format!(
            "backup start: {} / backup end: {} / backup duration: {}\n\
            total duration: {}",
            summary.backup_start.format("%Y-%m-%d %H:%M:%S"),
            summary.backup_end.format("%Y-%m-%d %H:%M:%S"),
            format_duration(std::time::Duration::from_secs_f64(summary.backup_duration)),
            format_duration(std::time::Duration::from_secs_f64(summary.total_duration))
        );
        add_entry("Duration", duration);
    }
    if let Some(ref description) = snap.description {
        add_entry("Description", description.clone());
    }
}
