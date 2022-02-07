use anyhow::{anyhow, Result};
use clap::Parser;

use crate::backend::{FileType, MapResult, ReadBackend};
use crate::blob::TreeIterator;
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
        let res = be.find_starts_with(FileType::Snapshot, &[&opts.id])?[0];
        match res {
            MapResult::Some(id) => Ok(id),
            MapResult::None => Err(anyhow!("no suitable id found for {}", &opts.id)),
            MapResult::NonUnique => Err(anyhow!("id {} is not unique", &opts.id)),
        }
    })?;

    let index= BoomIndex::from_iter(AllIndexFiles::new(be.clone()).into_iter());
    let snap = SnapshotFile::from_backend(be, id)?;

    for path_node in TreeIterator::from_id(be.clone(), index, snap.tree) {
        println!("{:?} ", path_node.path);
    }

    Ok(())
}
