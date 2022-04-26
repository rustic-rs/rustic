use anyhow::Result;
use bytesize::ByteSize;
use clap::Parser;
use humantime::format_duration;
use prettytable::{cell, format, row, Table};

use crate::backend::DecryptReadBackend;
use crate::repo::{SnapshotFile, SnapshotFilter, SnapshotGroup, SnapshotGroupCriterion};

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
                    let nodes = sn
                        .node_count
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "?".to_string());
                    let size = sn
                        .size
                        .map(|b| ByteSize(b).to_string_as(true))
                        .unwrap_or_else(|| "?".to_string());
                    row![sn.id, time, sn.hostname, tags, paths, r->nodes, r->size]
                })
                .collect();
            table.set_titles(
                row![b->"ID", b->"Time", b->"Host", b->"Tags", b->"Paths", br->"Nodes", br->"Size"],
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
    table.add_row(row![b->"Time", sn.time.format("%Y-%m-%d %H:%M:%S")]);
    table.add_row(row![b->"Host", sn.hostname]);
    table.add_row(row![b->"Tags", sn.tags.formatln()]);
    table.add_row(row![b->"Paths", sn.paths.formatln()]);
    table.add_row(row![]);
    table.add_row(row![b->"Command", sn.command.unwrap_or_else(|| "?".to_string())]);

    let source = format!(
        "size: {} / nodes: {}",
        sn.size
            .map(|b| ByteSize(b).to_string_as(true))
            .unwrap_or_else(|| "?".to_string()),
        sn.node_count
            .map(|c| c.to_string())
            .unwrap_or_else(|| "?".to_string()),
    );
    table.add_row(row![b->"Source", source]);

    table.add_row(row![]);

    let files = format!(
        "new: {:>10} / changed: {:>10} / unchanged: {:>10}",
        sn.files_new
            .map(|c| c.to_string())
            .unwrap_or_else(|| "?".to_string()),
        sn.files_changed
            .map(|c| c.to_string())
            .unwrap_or_else(|| "?".to_string()),
        sn.files_unchanged
            .map(|c| c.to_string())
            .unwrap_or_else(|| "?".to_string()),
    );
    table.add_row(row![b->"Files", files]);

    let trees = format!(
        "new: {:>10} / changed: {:>10} / unchanged: {:>10}",
        sn.trees_new
            .map(|c| c.to_string())
            .unwrap_or_else(|| "?".to_string()),
        sn.trees_changed
            .map(|c| c.to_string())
            .unwrap_or_else(|| "?".to_string()),
        sn.trees_unchanged
            .map(|c| c.to_string())
            .unwrap_or_else(|| "?".to_string()),
    );
    table.add_row(row![b->"Trees", trees]);

    table.add_row(row![]);

    let written = format!(
        "total: {} / tree blobs: {} / data blobs: {}",
        sn.data_added
            .map(|b| ByteSize(b).to_string_as(true))
            .unwrap_or_else(|| "?".to_string()),
        sn.tree_blobs_written
            .map(|c| c.to_string())
            .unwrap_or_else(|| "?".to_string()),
        sn.data_blobs_written
            .map(|c| c.to_string())
            .unwrap_or_else(|| "?".to_string()),
    );
    table.add_row(row![b->"Added to repo", written]);

    let duration = format!(
        "Start: {} / End: {} / Duration: {}",
        sn.backup_start
            .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "?".to_string()),
        sn.backup_end
            .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "?".to_string()),
        match (sn.backup_start, sn.backup_end) {
            (Some(start), Some(end)) =>
                format_duration((end - start).to_std().unwrap()).to_string(),
            _ => "?".to_string(),
        },
    );
    table.add_row(row![b->"Duration", duration]);

    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
    table.printstd();
    println!();
}
