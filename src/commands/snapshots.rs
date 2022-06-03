use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use humantime::format_duration;
use prettytable::{cell, format, row, Table};

use super::bytes;
use crate::backend::DecryptReadBackend;
use crate::repo::{
    DeleteOption, SnapshotFile, SnapshotFilter, SnapshotGroup, SnapshotGroupCriterion,
};

#[derive(Parser)]
pub(super) struct Opts {
    #[clap(flatten)]
    filter: SnapshotFilter,

    /// group snapshots by any combination of host,paths,tags
    #[clap(long, short = 'g', value_name = "CRITERION", default_value = "")]
    group_by: SnapshotGroupCriterion,

    /// show detailed information about snapshots
    #[clap(long)]
    long: bool,

    /// Snapshots to list
    #[clap(value_name = "ID")]
    ids: Vec<String>,
}

pub(super) async fn execute(be: &impl DecryptReadBackend, opts: Opts) -> Result<()> {
    let groups = match opts.ids.is_empty() {
        true => SnapshotFile::group_from_backend(be, &opts.filter, &opts.group_by).await?,
        false => vec![(
            SnapshotGroup::default(),
            SnapshotFile::from_ids(be, &opts.ids).await?,
        )],
    };

    for (group, mut snapshots) in groups {
        if !group.is_empty() {
            println!("\nsnapshots for {:?}", group);
        }
        snapshots.sort_unstable();
        let count = snapshots.len();

        if opts.long {
            for snap in snapshots {
                display_snap(snap);
            }
        } else {
            let mut table: Table = snapshots
                .into_iter()
                .map(|sn| {
                    let tags = sn.tags.formatln();
                    let paths = sn.paths.formatln();
                    let time = sn.time.format("%Y-%m-%d %H:%M:%S");
                    let (files, dirs, size) = sn
                        .summary
                        .map(|s| {
                            (
                                s.total_files_processed.to_string(),
                                s.total_dirs_processed.to_string(),
                                bytes(s.total_bytes_processed),
                            )
                        })
                        .unwrap_or_else(|| ("?".to_string(), "?".to_string(), "?".to_string()));
                    row![sn.id, time, sn.hostname, tags, paths, r->files, r->dirs, r->size]
                })
                .collect();
            table.set_titles(
                row![b->"ID", b->"Time", b->"Host", b->"Tags", b->"Paths", br->"Files",br->"Dirs", br->"Size"],
            );
            table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
            table.printstd();
        }
        println!("{} snapshot(s)", count);
    }

    Ok(())
}

fn display_snap(sn: SnapshotFile) {
    let mut table = Table::new();

    table.add_row(row![b->"Snapshot", b->sn.id.to_hex()]);
    // note that if original was not set, it is set to sn.id by the load process
    if sn.original != Some(sn.id) {
        table.add_row(row![b->"Original ID", sn.original.unwrap().to_hex()]);
    }
    table.add_row(row![b->"Time", sn.time.format("%Y-%m-%d %H:%M:%S")]);
    table.add_row(row![b->"Host", sn.hostname]);
    table.add_row(row![b->"Tags", sn.tags.formatln()]);
    let delete = match sn.delete {
        DeleteOption::NotSet => "not set".to_string(),
        DeleteOption::Never => "never".to_string(),
        DeleteOption::After(t) => format!("after {}", t.format("%Y-%m-%d %H:%M:%S")),
    };
    table.add_row(row![b->"Delete", delete]);
    table.add_row(row![b->"Paths", sn.paths.formatln()]);
    let parent = match sn.parent {
        None => "no parent snapshot".to_string(),
        Some(p) => p.to_hex(),
    };
    table.add_row(row![b->"Parent", parent]);
    if let Some(summary) = sn.summary {
        table.add_row(row![]);
        table.add_row(row![b->"Command", summary.command]);

        let source = format!(
            "files: {} / dirs: {} / size: {}",
            summary.total_files_processed,
            summary.total_dirs_processed,
            bytes(summary.total_bytes_processed)
        );
        table.add_row(row![b->"Source", source]);

        table.add_row(row![]);

        let files = format!(
            "new: {:>10} / changed: {:>10} / unchanged: {:>10}",
            summary.files_new, summary.files_changed, summary.files_unmodified,
        );
        table.add_row(row![b->"Files", files]);

        let trees = format!(
            "new: {:>10} / changed: {:>10} / unchanged: {:>10}",
            summary.dirs_new, summary.dirs_changed, summary.dirs_unmodified,
        );
        table.add_row(row![b->"Dirs", trees]);

        table.add_row(row![]);

        let written = format!(
            "data:  {:>10} blobs / raw: {:>10} / packed: {:>10}\n\
            tree:  {:>10} blobs / raw: {:>10} / packed: {:>10}\n\
            total: {:>10} blobs / raw: {:>10} / packed: {:>10}",
            summary.data_blobs,
            bytes(summary.data_added_files),
            bytes(summary.data_added_files_packed),
            summary.tree_blobs,
            bytes(summary.data_added_trees),
            bytes(summary.data_added_trees_packed),
            summary.tree_blobs + summary.data_blobs,
            bytes(summary.data_added),
            bytes(summary.data_added_packed),
        );
        table.add_row(row![b->"Added to repo", written]);

        let duration = format!(
            "backup start: {} / backup end: {} / backup duration: {}\n\
            total duration: {}",
            summary.backup_start.format("%Y-%m-%d %H:%M:%S"),
            summary.backup_end.format("%Y-%m-%d %H:%M:%S"),
            format_duration(Duration::from_secs_f64(summary.backup_duration)),
            format_duration(Duration::from_secs_f64(summary.total_duration))
        );
        table.add_row(row![b->"Duration", duration]);
    }
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
    table.printstd();
    println!();
}
