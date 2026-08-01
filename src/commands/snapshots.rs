//! `snapshot` subcommand

use crate::{
    Application, RUSTIC_APP,
    helpers::{bold_cell, bytes_size_to_string, table, table_with_titles},
    repository::{OpenRepo, get_global_grouped_snapshots},
    status_err,
};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use comfy_table::{Cell, CellAlignment};
use derive_more::From;
use itertools::Itertools;
use jiff::SignedDuration;

use rustic_core::{
    Group, ProgressBars, ProgressType, SnapshotGroup,
    repofile::{DeleteOption, SnapshotFile},
};
use serde::Serialize;

#[cfg(feature = "tui")]
use crate::commands::tui;

#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
enum SnapshotColumn {
    Id,
    Time,
    Host,
    Label,
    Tags,
    Paths,
    Files,
    Dirs,
    Size,
}

const DEFAULT_SNAPSHOT_COLUMNS: [SnapshotColumn; 9] = [
    SnapshotColumn::Id,
    SnapshotColumn::Time,
    SnapshotColumn::Host,
    SnapshotColumn::Label,
    SnapshotColumn::Tags,
    SnapshotColumn::Paths,
    SnapshotColumn::Files,
    SnapshotColumn::Dirs,
    SnapshotColumn::Size,
];

impl SnapshotColumn {
    const fn index(self) -> usize {
        match self {
            Self::Id => 0,
            Self::Time => 1,
            Self::Host => 2,
            Self::Label => 3,
            Self::Tags => 4,
            Self::Paths => 5,
            Self::Files => 6,
            Self::Dirs => 7,
            Self::Size => 8,
        }
    }

    const fn title(self) -> &'static str {
        match self {
            Self::Id => "ID",
            Self::Time => "Time",
            Self::Host => "Host",
            Self::Label => "Label",
            Self::Tags => "Tags",
            Self::Paths => "Paths",
            Self::Files => "Files",
            Self::Dirs => "Dirs",
            Self::Size => "Size",
        }
    }

    const fn is_numeric(self) -> bool {
        matches!(self, Self::Files | Self::Dirs | Self::Size)
    }
}

/// `snapshot` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct SnapshotCmd {
    /// Snapshots to show. If none is given, use filter options to filter from all snapshots
    ///
    /// Snapshots can be identified the following ways: "01a2b3c4" or "latest" or "latest~N" (N >= 0)
    #[clap(value_name = "ID")]
    ids: Vec<String>,

    /// Show detailed information about snapshots
    #[arg(long)]
    long: bool,

    /// Show snapshots in json format
    #[clap(long, conflicts_with = "long")]
    json: bool,

    /// Show all snapshots instead of summarizing identical follow-up snapshots
    #[clap(long, conflicts_with_all = &["long", "json"])]
    all: bool,

    /// Comma-separated columns to show in tabular output
    ///
    /// Available columns: id, time, host, label, tags, paths, files, dirs, size
    #[clap(
        long,
        value_enum,
        value_delimiter = ',',
        value_name = "COLUMN",
        conflicts_with_all = &["long", "json"]
    )]
    columns: Vec<SnapshotColumn>,

    #[cfg(feature = "tui")]
    /// Run in interactive UI mode
    #[clap(long, short)]
    pub interactive: bool,
}

impl Runnable for SnapshotCmd {
    fn run(&self) {
        #[cfg(feature = "tui")]
        let result = if self.interactive {
            // Opening and indexing can ask for a password. Do that before entering raw mode
            // so dialoguer's prompt remains usable on an interactive terminal.
            RUSTIC_APP
                .config()
                .repository
                .run_indexed(|repo| self.interactive_run(repo))
        } else {
            RUSTIC_APP
                .config()
                .repository
                .run_open(|repo| self.inner_run(repo))
        };

        #[cfg(not(feature = "tui"))]
        let result = RUSTIC_APP
            .config()
            .repository
            .run_open(|repo| self.inner_run(repo));

        if let Err(err) = result {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl SnapshotCmd {
    #[cfg(feature = "tui")]
    fn interactive_run(&self, repo: crate::repository::IndexedRepo) -> Result<()> {
        let config = RUSTIC_APP.config();

        tui::run(|progress| {
            let p = progress.progress(
                ProgressType::Spinner,
                "starting rustic in interactive mode...",
            );
            let snapshots = tui::Snapshots::new(
                &repo,
                config.snapshot_filter.clone(),
                config.global.group_by.unwrap_or_default(),
            )?;
            p.finish();
            tui::run_app(progress.terminal, snapshots)
        })
    }

    fn inner_run(&self, repo: OpenRepo) -> Result<()> {
        let groups = get_global_grouped_snapshots(&repo, &self.ids)?.groups;

        if self.json {
            let mut stdout = std::io::stdout();
            if groups.len() == 1 && groups[0].group_key.is_empty() {
                // we don't use grouping, only output snapshots list
                serde_json::to_writer(&mut stdout, &groups[0].items)?;
            } else {
                #[derive(Serialize, From)]
                struct SnapshotsGroup {
                    group_key: SnapshotGroup,
                    snapshots: Vec<SnapshotFile>,
                }
                let groups: Vec<SnapshotsGroup> = groups
                    .into_iter()
                    .map(|g| (g.group_key, g.items).into())
                    .collect();
                serde_json::to_writer(&mut stdout, &groups)?;
            }
            return Ok(());
        }

        let mut total_count = 0;
        for Group { group_key, items } in groups {
            if !group_key.is_empty() {
                println!("\nsnapshots for {group_key}");
            }
            total_count += items.len();
            print_snapshots_with_columns(items, self.long, self.all, &self.columns);
        }
        println!();
        println!("total: {total_count} snapshot(s)");

        Ok(())
    }
}

pub fn print_snapshots(snapshots: Vec<SnapshotFile>, long: bool, all: bool) {
    print_snapshots_with_columns(snapshots, long, all, &[]);
}

fn print_snapshots_with_columns(
    snapshots: Vec<SnapshotFile>,
    long: bool,
    all: bool,
    columns: &[SnapshotColumn],
) {
    let count = snapshots.len();
    if long {
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
        let columns = if columns.is_empty() {
            &DEFAULT_SNAPSHOT_COLUMNS
        } else {
            columns
        };
        let mut table = table_with_titles(columns.iter().map(|column| column.title()));
        for (index, _) in columns
            .iter()
            .enumerate()
            .filter(|(_, column)| column.is_numeric())
        {
            if let Some(table_column) = table.column_iter_mut().nth(index) {
                table_column.set_cell_alignment(CellAlignment::Right);
            }
        }

        if all {
            // Add all snapshots to output table
            _ = table.add_rows(
                snapshots
                    .into_iter()
                    .map(|sn| snap_to_table_columns(&sn, 0, columns)),
            );
        } else {
            // Group snapshts by treeid and output into table
            _ = table.add_rows(
                snapshots
                    .into_iter()
                    .chunk_by(|sn| sn.tree)
                    .into_iter()
                    .map(|(_, mut g)| {
                        snap_to_table_columns(&g.next().unwrap(), g.count(), columns)
                    }),
            );
        }
        println!("{table}");
    }
    println!("{count} snapshot(s)");
}

fn snap_to_table_columns(
    sn: &SnapshotFile,
    count: usize,
    columns: &[SnapshotColumn],
) -> Vec<String> {
    let values = snap_to_table(sn, count);
    columns
        .iter()
        .map(|column| values[column.index()].clone())
        .collect()
}

pub fn snap_to_table(sn: &SnapshotFile, count: usize) -> [String; 9] {
    let config = RUSTIC_APP.config();
    let tags = sn.tags.formatln();
    let paths = sn.paths.formatln();
    let time = config.global.format_time(&sn.time);
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
    let config = RUSTIC_APP.config();
    add_entry("Snapshot", snap.id.to_hex().to_string());
    // note that if original was not set, it is set to snap.id by the load process
    if let Some(original) = snap.original
        && original != snap.id
    {
        add_entry("Original ID", original.to_hex().to_string());
    }
    add_entry("Time", config.global.format_time(&snap.time).to_string());
    add_entry("Generated by", snap.program_version.clone());
    add_entry("Host", snap.hostname.clone());
    add_entry("Label", snap.label.clone());
    add_entry("Tags", snap.tags.formatln());
    let delete = match &snap.delete {
        DeleteOption::NotSet => "not set".to_string(),
        DeleteOption::Never => "never".to_string(),
        DeleteOption::After(t) => format!("after {}", config.global.format_time(t)),
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
            "backup start: {} / backup end: {} / backup duration: {:#}\n\
            total duration: {:#}",
            config.global.format_time(&summary.backup_start),
            config.global.format_time(&summary.backup_end),
            SignedDuration::from_secs_f64(summary.backup_duration),
            SignedDuration::from_secs_f64(summary.total_duration),
        );
        add_entry("Duration", duration);
    }
    if let Some(ref description) = snap.description {
        add_entry("Description", normalize_line_endings(description));
    }
}

/// Normalize snapshot text before passing it to terminal table rendering.
///
/// Snapshot descriptions are stored verbatim, including CRLF endings from
/// `--description-from` on Windows. A raw carriage return makes terminal
/// output overwrite the start of its table row.
fn normalize_line_endings(value: &str) -> String {
    value.replace("\r\n", "\n").replace('\r', "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn description_with_windows_line_endings_renders_each_line() {
        let mut table = table();
        _ = table.add_row([
            bold_cell("Description"),
            Cell::new(normalize_line_endings("first line\r\nsecond line\r\n")),
        ]);

        let output = table.to_string();
        assert!(output.contains("| Description | first line"));
        assert!(output.contains("|             | second line"));
        assert!(!output.contains('\r'));
    }

    #[test]
    fn line_ending_normalization_keeps_empty_lines() {
        assert_eq!(
            normalize_line_endings("first\r\n\r\nthird\r"),
            "first\n\nthird\n"
        );
    }

    #[test]
    fn selected_snapshot_columns_keep_the_requested_order() {
        let values = [
            "id".to_string(),
            "time".to_string(),
            "host".to_string(),
            "label".to_string(),
            "tags".to_string(),
            "paths".to_string(),
            "files".to_string(),
            "dirs".to_string(),
            "size".to_string(),
        ];
        let columns = [
            SnapshotColumn::Size,
            SnapshotColumn::Id,
            SnapshotColumn::Host,
        ];
        let selected: Vec<_> = columns
            .into_iter()
            .map(|column| values[column.index()].clone())
            .collect();

        assert_eq!(selected, ["size", "id", "host"]);
    }
}
