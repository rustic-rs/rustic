use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use humantime::format_duration;
use itertools::Itertools;
use prettytable::{format, row, Table};

use super::{bytes, RusticConfig};
use crate::backend::DecryptReadBackend;
use crate::repo::{
    DeleteOption, SnapshotFile, SnapshotFilter, SnapshotGroup, SnapshotGroupCriterion,
};

#[derive(Parser)]
pub(super) struct Opts {
    #[clap(flatten, help_heading = "SNAPSHOT FILTER OPTIONS")]
    filter: SnapshotFilter,

    /// Group snapshots by any combination of host,paths,tags
    #[clap(
        long,
        short = 'g',
        value_name = "CRITERION",
        default_value = "host,paths"
    )]
    group_by: SnapshotGroupCriterion,

    /// Show detailed information about snapshots
    #[clap(long)]
    long: bool,

    /// Show all snapshots instead of summarizing identical follow-up snapshots
    #[clap(long)]
    all: bool,

    /// Snapshots to show
    #[clap(value_name = "ID")]
    ids: Vec<String>,
}

pub(super) async fn execute(
    be: &impl DecryptReadBackend,
    mut opts: Opts,
    config_file: RusticConfig,
) -> Result<()> {
    config_file.merge_into("snapshot-filter", &mut opts.filter)?;

    let groups = match &opts.ids[..] {
        [] => SnapshotFile::group_from_backend(be, &opts.filter, &opts.group_by).await?,
        [id] if id == "latest" => {
            SnapshotFile::group_from_backend(be, &opts.filter, &opts.group_by)
                .await?
                .into_iter()
                .map(|(group, mut snaps)| {
                    snaps.sort_unstable();
                    let last_idx = snaps.len() - 1;
                    snaps.swap(0, last_idx);
                    snaps.truncate(1);
                    (group, snaps)
                })
                .collect::<Vec<_>>()
        }
        _ => vec![(
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
            let snap_to_table = |(sn, count): (SnapshotFile, usize)| {
                let tags = sn.tags.formatln();
                let paths = sn.paths.formatln();
                let time = sn.time.format("%Y-%m-%d %H:%M:%S");
                let (files, dirs, size) = match &sn.summary {
                    Some(s) => (
                        s.total_files_processed.to_string(),
                        s.total_dirs_processed.to_string(),
                        bytes(s.total_bytes_processed),
                    ),
                    None => ("?".to_string(), "?".to_string(), "?".to_string()),
                };
                let id = match count {
                    0 => format!("{}", sn.id),
                    count => format!("{} (+{})", sn.id, count),
                };
                row![id, time, sn.hostname, tags, paths, r->files, r->dirs, r->size]
            };

            let mut table: Table = snapshots
                .into_iter()
                .group_by(|sn| if opts.all { sn.id } else { sn.tree })
                .into_iter()
                .map(|(_, mut g)| (g.next().unwrap(), g.count()))
                .map(snap_to_table)
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
