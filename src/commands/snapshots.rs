use anyhow::Result;
use clap::Parser;
use prettytable::{cell, format, row, Table};

use crate::backend::{FileType, ReadBackend};
use crate::repo::SnapshotFile;

#[derive(Parser)]
pub(super) struct Opts {}

pub(super) fn execute(be: &impl ReadBackend, _opts: Opts) -> Result<()> {
    let mut table = Table::new();
    table.set_titles(row!["ID", "Time", "Host", "Tags", "Paths"]);
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);

    for id in be.list(FileType::Snapshot)? {
        let sn = SnapshotFile::from_backend(be, id)?;
        let paths = sn
            .paths
            .iter()
            .map(|p| p.to_string_lossy() + "\n")
            .collect::<String>();
        table.add_row(row![id, sn.time, sn.hostname, "", paths,]);
    }
    table.printstd();

    Ok(())
}
