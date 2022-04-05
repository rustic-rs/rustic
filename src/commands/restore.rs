use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::Parser;
use derive_getters::Dissolve;
use futures::StreamExt;
use vlog::*;

use super::progress_counter;
use crate::backend::{DecryptReadBackend, FileType, LocalBackend};
use crate::blob::{Node, NodeType, TreeStreamer};
use crate::id::Id;
use crate::index::{IndexBackend, IndexedBackend};
use crate::repo::SnapshotFile;

#[derive(Parser)]
pub(super) struct Opts {
    /// dry-run: don't restore, only show what would be done
    #[clap(long, short = 'n')]
    dry_run: bool,

    /// TODO: remove files/dirs destination which are not contained in snapshot
    #[clap(long)]
    delete: bool,

    /// snapshot to restore
    id: String,

    /// restore destination
    dest: String,
}

pub(super) async fn execute(be: &(impl DecryptReadBackend + Unpin), opts: Opts) -> Result<()> {
    let snap = SnapshotFile::from_str(be, &opts.id, |_| true, progress_counter()).await?;

    let dest = LocalBackend::new(&opts.dest);
    let index = IndexBackend::new(be, progress_counter()).await?;

    v2!("1st tree walk: allocating dirs/files and collecting restore information...");
    let file_infos = allocate_and_collect(&dest, index.clone(), snap.tree, &opts).await?;

    v2!("restoring file contents...");
    restore_contents(be, &dest, file_infos, &opts).await?;

    v2!("2nd tree walk: setting metadata");
    restore_metadata(&dest, index, snap.tree, &opts).await?;

    v1!("done.");
    Ok(())
}

/// allocate files, scan or remove existing files and collect restore information
async fn allocate_and_collect(
    dest: &LocalBackend,
    index: impl IndexedBackend + Unpin,
    tree: Id,
    opts: &Opts,
) -> Result<FileInfos> {
    let mut file_infos = FileInfos::new();

    let mut tree_streamer = TreeStreamer::new(index.clone(), vec![tree], false).await?;
    while let Some(item) = tree_streamer.next().await {
        let (path, node) = item?;
        match node.node_type() {
            NodeType::Dir => {
                if !opts.dry_run {
                    dest.create_dir(&path);
                }
            }
            NodeType::File => {
                // collect blobs needed for restoring
                let size = file_infos.add_file(&node, path.clone(), &index)?;
                // create the file
                if !opts.dry_run {
                    dest.create_file(&path, size);
                }
            }
            _ => {} // nothing to do for symlink, device, etc.
        }
    }

    Ok(file_infos)
}

/// restore_contents restores all files contents as described by file_infos
/// using the ReadBackend be and writing them into the LocalBackend dest.
async fn restore_contents(
    be: &impl DecryptReadBackend,
    dest: &LocalBackend,
    file_infos: FileInfos,
    opts: &Opts,
) -> Result<()> {
    let (filenames, restore_info) = file_infos.dissolve();
    for (pack, blob) in restore_info {
        for (bl, fls) in blob {
            // read pack at blob_offset with length blob_length
            let data = be
                .read_encrypted_partial(FileType::Pack, &pack, bl.offset, bl.length)
                .await?;
            for fl in fls {
                // save in file at file_start
                if !opts.dry_run {
                    dest.write_at(&filenames[fl.file_idx], fl.file_start, &data);
                }
            }
        }
    }
    Ok(())
}

async fn restore_metadata(
    dest: &LocalBackend,
    index: impl IndexedBackend + Unpin,
    tree: Id,
    opts: &Opts,
) -> Result<()> {
    // walk over tree in repository and compare with tree in dest
    let mut tree_streamer = TreeStreamer::new(index, vec![tree], false).await?;
    while let Some(item) = tree_streamer.next().await {
        let (path, node) = item?;
        if !opts.dry_run {
            if let NodeType::Symlink { linktarget } = node.node_type() {
                dest.create_symlink(&path, linktarget);
            }
            dest.set_metadata(&path, node.meta());
        }
    }

    Ok(())
}
/// struct that contains information of file contents grouped by
/// 1) pack ID,
/// 2) blob within this pack
/// 3) the actual files and position of this blob within those
#[derive(Debug, Dissolve)]
struct FileInfos {
    names: Filenames,
    r: RestoreInfo,
}

type RestoreInfo = HashMap<Id, HashMap<BlobLocation, Vec<FileLocation>>>;
type Filenames = Vec<PathBuf>;

#[derive(Debug, Hash, PartialEq, Eq)]
struct BlobLocation {
    offset: u32,
    length: u32,
}

#[derive(Debug)]
struct FileLocation {
    file_idx: usize,
    file_start: u64,
}

impl FileInfos {
    fn new() -> Self {
        Self {
            names: Vec::new(),
            r: HashMap::new(),
        }
    }

    /// Add the file to FilesInfos using index to get blob information.
    /// Returns the computed length of the file
    fn add_file(&mut self, file: &Node, name: PathBuf, index: &impl IndexedBackend) -> Result<u64> {
        let mut file_pos = 0;
        if !file.content().is_empty() {
            let file_idx = self.names.len();
            self.names.push(name);
            for id in file.content().iter() {
                let ie = index
                    .get_data(id)
                    .ok_or_else(|| anyhow!("did not find id {} in index", id))?;
                let bl = BlobLocation {
                    offset: *ie.offset(),
                    length: *ie.length(),
                };

                let pack = self.r.entry(*ie.pack()).or_insert_with(HashMap::new);
                let blob_location = pack.entry(bl).or_insert_with(Vec::new);
                blob_location.push(FileLocation {
                    file_idx,
                    file_start: file_pos,
                });

                file_pos += *ie.length() as u64 - 32; // blob crypto overhead
            }
        }
        Ok(file_pos)
    }
}
