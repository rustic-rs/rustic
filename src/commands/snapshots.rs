use anyhow::Result;
use bytesize::ByteSize;
use clap::Parser;
use prettytable::{cell, format, row, Table};

use crate::backend::ReadBackend;
use crate::repo::SnapshotFile;

#[derive(Parser)]
pub(super) struct Opts {}

pub(super) fn execute(be: &impl ReadBackend, _opts: Opts) -> Result<()> {
    let mut snapshots = SnapshotFile::all_from_backend(be)?;
    snapshots.sort();

    let mut table = Table::new();
    table.set_titles(
        row![b->"ID", b->"Time", b->"Host", b->"Tags", b->"Paths", br->"Files", br->"Size"],
    );
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
    for sn in snapshots {
        let paths = sn.paths.into_iter().map(|p| p + "\n").collect::<String>();
        let time = sn.time.format("%Y-%m-%d %H:%M:%S");
        let size = sn
            .size
            .map(|b| ByteSize(b).to_string_as(true))
            .unwrap_or("?".to_string());
        let files = sn
            .file_count
            .map(|c| c.to_string())
            .unwrap_or("?".to_string());
        table.add_row(row![sn.id, time, sn.hostname, "", paths, r->files, r->size]);
    }
    table.printstd();

    Ok(())
}
