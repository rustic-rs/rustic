use anyhow::Result;
use clap::Parser;
use itertools::{
    EitherOrBoth::{Both, Left, Right},
    Itertools,
};
use vlog::*;

use super::progress_counter;
use crate::backend::{DecryptReadBackend, FileType};
use crate::blob::{tree_iterator, NodeType};
use crate::id::Id;
use crate::index::IndexBackend;
use crate::repo::SnapshotFile;

#[derive(Parser)]
pub(super) struct Opts {
    /// reference snapshot
    id1: String,

    /// new snapshot
    id2: String,
}

pub(super) fn execute(be: &impl DecryptReadBackend, opts: Opts) -> Result<()> {
    v1!("getting snapshots...");
    let ids = match (Id::from_hex(&opts.id1), Id::from_hex(&opts.id2)) {
        (Ok(id1), Ok(id2)) => vec![id1, id2],
        // if the given id param are not full Ids, search for a suitable one
        _ => be
            .find_starts_with(FileType::Snapshot, &[&opts.id1, &opts.id2])?
            .into_iter()
            .collect::<Result<Vec<_>>>()?,
    };

    let snap = SnapshotFile::from_backend(be, &ids[0])?;
    let snap_with = SnapshotFile::from_backend(be, &ids[1])?;

    let index = IndexBackend::new(be, progress_counter())?;
    let iterator1 = tree_iterator(&index, vec![snap.tree])?.filter_map(Result::ok);
    let iterator2 = tree_iterator(&index, vec![snap_with.tree])?.filter_map(Result::ok);

    for file in iterator1.merge_join_by(iterator2, |(path1, _), (path2, _)| path1.cmp(path2)) {
        match file {
            Left((path, _)) => println!("-    {:?}", path),
            Right((path, _)) => println!("+    {:?}", path),
            Both((path, node1), (_, node2)) => match node1.node_type() {
                tpe if tpe != node2.node_type() => println!("M    {:?}", path), // type was changed
                NodeType::File if node1.content() != node2.content() => println!("M    {:?}", path),
                NodeType::Symlink { linktarget } => {
                    if let NodeType::Symlink {
                        linktarget: linktarget2,
                    } = node2.node_type()
                    {
                        if *linktarget != *linktarget2 {
                            println!("M    {:?}", path)
                        }
                    }
                }
                _ => {} // no difference to show
            },
        }
    }

    Ok(())
}
