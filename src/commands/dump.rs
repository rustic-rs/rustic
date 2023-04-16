use anyhow::{bail, Result};
use clap::Parser;
use std::io::Write;
use std::path::Path;

use crate::blob::{BlobType, NodeType, Tree};
use crate::index::{IndexBackend, IndexedBackend};
use crate::repofile::{SnapshotFile, SnapshotFilter};
use crate::repository::OpenRepository;

use super::{progress_counter, RusticConfig};

#[derive(Parser)]
pub(super) struct Opts {
    /// file from snapshot to dump
    #[clap(value_name = "SNAPSHOT[:PATH]")]
    snap: String,

    #[clap(
        flatten,
        next_help_heading = "Snapshot filter options (when using latest)"
    )]
    filter: SnapshotFilter,
}

pub(super) fn execute(
    repo: OpenRepository,
    mut opts: Opts,
    config_file: RusticConfig,
) -> Result<()> {
    config_file.merge_into("snapshot-filter", &mut opts.filter)?;
    let be = &repo.dbe;

    let (id, path) = opts.snap.split_once(':').unwrap_or((&opts.snap, ""));
    let snap = SnapshotFile::from_str(be, id, |sn| sn.matches(&opts.filter), progress_counter(""))?;
    let index = IndexBackend::new(be, progress_counter(""))?;
    let node = Tree::node_from_path(&index, snap.tree, Path::new(path))?;

    if node.node_type != NodeType::File {
        bail!("dump only supports regular files!");
    }

    let mut stdout = std::io::stdout();
    for id in node.content.unwrap() {
        // TODO: cache blobs which are needed later
        let data = index.blob_from_backend(BlobType::Data, &id)?;
        stdout.write_all(&data)?;
    }

    Ok(())
}
