use anyhow::Result;
use clap::Parser;

use super::progress_counter;
use crate::backend::DecryptReadBackend;
use crate::blob::tree_iterator;
use crate::index::IndexBackend;
use crate::repo::SnapshotFile;

#[derive(Parser)]
pub(super) struct Opts {
    /// snapshot to ls
    id: String,
}

pub(super) fn execute(be: &impl DecryptReadBackend, opts: Opts) -> Result<()> {
    let snap = SnapshotFile::from_str(be, &opts.id, |_| true, progress_counter())?;
    let index = IndexBackend::new(be, progress_counter())?;

    let tree_iter = tree_iterator(&index, vec![snap.tree])?.filter_map(Result::ok);
    for (path, _) in tree_iter {
        println!("{:?} ", path);
    }

    Ok(())
}
