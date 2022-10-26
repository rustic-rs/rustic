use std::path::Path;

use anyhow::Result;
use clap::Parser;

use super::progress_counter;
use crate::backend::DecryptReadBackend;
use crate::blob::{NodeStreamer, NodeType, Tree};
use crate::commands::helpers::progress_spinner;
use crate::index::IndexBackend;
use crate::repo::SnapshotFile;

#[derive(Parser)]
pub(super) struct Opts {
    /// Reference snapshot/path
    #[clap(value_name = "SNAPSHOT1[:PATH1]")]
    snap1: String,

    /// New snapshot/path [default for PATH2: PATH1]
    #[clap(value_name = "SNAPSHOT2[:PATH2]")]
    snap2: String,
}

pub(super) fn execute(be: &(impl DecryptReadBackend + Unpin), opts: Opts) -> Result<()> {
    let (id1, path1) = opts.snap1.split_once(':').unwrap_or((&opts.snap1, ""));
    let (id2, path2) = opts.snap2.split_once(':').unwrap_or((&opts.snap2, path1));

    let p = progress_spinner("getting snapshots...");
    p.finish();
    let snaps = SnapshotFile::from_ids(be, &[id1.to_string(), id2.to_string()])?;

    let snap1 = &snaps[0];
    let snap2 = &snaps[1];

    let index = IndexBackend::new(be, progress_counter(""))?;
    let id1 = Tree::subtree_id(&index, snap1.tree, Path::new(path1))?;
    let id2 = Tree::subtree_id(&index, snap2.tree, Path::new(path2))?;

    let mut tree_streamer1 = NodeStreamer::new(index.clone(), id1)?;
    let mut tree_streamer2 = NodeStreamer::new(index, id2)?;

    let mut item1 = tree_streamer1.next().transpose()?;
    let mut item2 = tree_streamer2.next().transpose()?;

    loop {
        match (&item1, &item2) {
            (None, None) => break,
            (Some(i1), None) => {
                println!("-    {:?}", i1.0);
                item1 = tree_streamer1.next().transpose()?;
            }
            (None, Some(i2)) => {
                println!("+    {:?}", i2.0);
                item2 = tree_streamer2.next().transpose()?;
            }
            (Some(i1), Some(i2)) if i1.0 < i2.0 => {
                println!("-    {:?}", i1.0);
                item1 = tree_streamer1.next().transpose()?;
            }
            (Some(i1), Some(i2)) if i1.0 > i2.0 => {
                println!("+    {:?}", i2.0);
                item2 = tree_streamer2.next().transpose()?;
            }
            (Some(i1), Some(i2)) => {
                let path = &i1.0;
                let node1 = &i1.1;
                let node2 = &i2.1;
                match node1.node_type() {
                    tpe if tpe != node2.node_type() => println!("M    {:?}", path), // type was changed
                    NodeType::File if node1.content() != node2.content() => {
                        println!("M    {:?}", path)
                    }
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
                }
                item1 = tree_streamer1.next().transpose()?;
                item2 = tree_streamer2.next().transpose()?;
            }
        }
    }

    Ok(())
}
