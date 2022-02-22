use anyhow::Result;
use clap::Parser;
use std::collections::HashMap;

use crate::backend::{DecryptReadBackend, FileType};
use crate::blob::{tree_iterator_once, NodeType};
use crate::index::{AllIndexFiles, BoomIndex, ReadIndex};
use crate::repo::{IndexBlob, SnapshotFile};

#[derive(Parser)]
pub(super) struct Opts {
    /// read all data blobs
    #[clap(long)]
    read_data: bool,
}

pub(super) fn execute(be: &impl DecryptReadBackend, opts: Opts) -> Result<()> {
    println!("checking packs...");
    check_packs(be)?;

    println!("loading index...");
    let index = BoomIndex::from_iter(AllIndexFiles::new(be.clone()).into_iter());

    println!("checking snapshots and trees...");
    check_snapshots(be, &index)?;

    if opts.read_data {
        unimplemented!()
    }

    Ok(())
}

// calculate the pack size from the contained blobs
fn pack_size(blobs: &[IndexBlob]) -> u32 {
    let mut size = 4 + 32; // 4 + crypto overhead
    for blob in blobs {
        size += blob.length() + 37 // 37 = length of blob description
    }
    size
}

// check if packs correspond to index
fn check_packs(be: &impl DecryptReadBackend) -> Result<()> {
    let mut packs = AllIndexFiles::new(be.clone())
        .into_iter()
        .map(|p| (*p.id(), pack_size(p.blobs())))
        .collect::<HashMap<_, _>>();

    for (id, size) in be.list_with_size(FileType::Pack)? {
        match packs.remove(&id) {
            None => println!("pack {} not contained in index", id.to_hex()),
            Some(index_size) if index_size != size => println!(
                "pack {}: size computed by index: {}, actual size: {}",
                id.to_hex(),
                index_size,
                size
            ),
            _ => {} //everything ok
        }
    }

    for (id, _) in packs {
        println!(
            "pack {} is referenced by the index but not presend!",
            id.to_hex()
        );
    }

    Ok(())
}

// check if all snapshots and contained trees can be loaded and contents exist in the index
fn check_snapshots(be: &impl DecryptReadBackend, index: &impl ReadIndex) -> Result<()> {
    let snap_ids = be
        .list(FileType::Snapshot)?
        .into_iter()
        .map(|id| SnapshotFile::from_backend(be, id).unwrap().tree)
        .collect();

    for (path, node) in tree_iterator_once(be, index, snap_ids) {
        match node.node_type() {
            NodeType::File => {
                for (i, id) in node.content().iter().enumerate() {
                    if id.is_null() {
                        println!("file {:?} blob {} has null ID", path, i);
                    }

                    if index.get_id(id).is_none() {
                        println!("file {:?} blob {} is missig in index", path, id);
                    }
                }
            }

            NodeType::Dir => {
                match node.subtree() {
                    None => println!("dir {:?} subtree does not exist", path),
                    Some(tree) if tree.is_null() => println!("dir {:?} subtree has null ID", path),
                    _ => {} // subtree is ok
                }
            }

            _ => {} // nothing to check
        }
    }

    Ok(())
}
