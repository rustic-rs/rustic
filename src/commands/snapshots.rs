use anyhow::Result;
use bytesize::ByteSize;
use clap::Parser;
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
            println!("snapshots for {:?}", group);
        }
        snapshots.sort_unstable();
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
        let count = table.len();
        table.set_titles(
            row![b->"ID", b->"Time", b->"Host", b->"Tags", b->"Paths", br->"Nodes", br->"Size"],
        );
        table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
        table.printstd();
        println!("{} snapshot(s)", count);
    }

    Ok(())
}
