use anyhow::Result;
use clap::Parser;

use crate::backend::{FileType, ReadBackend};
use crate::blob::tree_iterator;
use crate::id::Id;
use crate::index::{AllIndexFiles, BoomIndex};
use crate::repo::SnapshotFile;

#[derive(Parser)]
pub(super) struct Opts {
    /// snapshot to ls
    id: String,
}

pub(super) fn execute(be: &impl ReadBackend, opts: Opts) -> Result<()> {
    let id = Id::from_hex(&opts.id).or_else(|_| {
        // if the given id param is not a full Id, search for a suitable one
        be.find_starts_with(FileType::Index, &[&opts.id])?.remove(0)
    })?;

    let index = BoomIndex::from_iter(AllIndexFiles::new(be.clone()).into_iter());
    let snap = SnapshotFile::from_backend(be, id)?;

    for (path, _) in tree_iterator(be, &index, vec![snap.tree]) {
        println!("{:?} ", path);
    }

    Ok(())
}
