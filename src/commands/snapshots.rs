//! `smapshot` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{
    commands::open_repository,
    helpers::{bold_cell, bytes_size_to_string, table, table_right_from},
    status_err, Application, RUSTIC_APP,
};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use comfy_table::Cell;
use humantime::format_duration;
use itertools::Itertools;

use rustic_core::{
    repofile::{DeleteOption, SnapshotFile},
    SnapshotGroupCriterion,
};

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
}
impl Runnable for SnapshotCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl SnapshotCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();
        let repo = open_repository(&config)?;

        let groups = repo.get_snapshot_group(&self.ids, self.group_by, |sn| {
            config.snapshot_filter.matches(sn)
        })?;

        if self.json {
            let mut stdout = std::io::stdout();
            serde_json::to_writer_pretty(&mut stdout, &groups)?;
            return Ok(());
        }

        let mut total_count = 0;
        for (group, mut snapshots) in groups {
            if !group.is_empty() {
                println!("\nsnapshots for {group}");
            }
            snapshots.sort_unstable();
            let count = snapshots.len();

            if self.long {
                for snap in snapshots {
                    snap.print_table();
                }
            } else {
                let snap_to_table = |(sn, count): (SnapshotFile, usize)| {
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
                        sn.hostname,
                        sn.label,
                        tags,
                        paths,
                        files,
                        dirs,
                        size,
                    ]
                };

                let mut table = table_right_from(
                    6,
                    [
                        "ID", "Time", "Host", "Label", "Tags", "Paths", "Files", "Dirs", "Size",
                    ],
                );

                let snapshots: Vec<_> = snapshots
                    .into_iter()
                    .group_by(|sn| if self.all { sn.id } else { sn.tree })
                    .into_iter()
                    .map(|(_, mut g)| (g.next().unwrap(), g.count()))
                    .map(snap_to_table)
                    .collect();
                _ = table.add_rows(snapshots);
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

trait PrintTable {
    fn print_table(&self);
}

impl PrintTable for SnapshotFile {
    fn print_table(&self) {
        let mut table = table();

        let mut add_entry = |title: &str, value: String| {
            _ = table.add_row([bold_cell(title), Cell::new(value)]);
        };

        add_entry("Snapshot", self.id.to_hex().to_string());
        // note that if original was not set, it is set to self.id by the load process
        if self.original != Some(self.id) {
            add_entry("Original ID", self.original.unwrap().to_hex().to_string());
        }
        add_entry("Time", self.time.format("%Y-%m-%d %H:%M:%S").to_string());
        add_entry("Generated by", self.program_version.clone());
        add_entry("Host", self.hostname.clone());
        add_entry("Label", self.label.clone());
        add_entry("Tags", self.tags.formatln());
        let delete = match self.delete {
            DeleteOption::NotSet => "not set".to_string(),
            DeleteOption::Never => "never".to_string(),
            DeleteOption::After(t) => format!("after {}", t.format("%Y-%m-%d %H:%M:%S")),
        };
        add_entry("Delete", delete);
        add_entry("Paths", self.paths.formatln());
        let parent = self.parent.map_or_else(
            || "no parent snapshot".to_string(),
            |p| p.to_hex().to_string(),
        );
        add_entry("Parent", parent);
        if let Some(ref summary) = self.summary {
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
        if let Some(ref description) = self.description {
            add_entry("Description", description.clone());
        }

        println!("{table}");
        println!();
    }
}
