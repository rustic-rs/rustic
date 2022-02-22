use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use derive_getters::Dissolve;
use itertools::{
    EitherOrBoth::{Both, Left, Right},
    Itertools,
};

use crate::backend::{DecryptReadBackend, FileType, LocalBackend};
use crate::blob::{tree_iterator, Node, NodeType};
use crate::id::Id;
use crate::index::{IndexBackend, IndexedBackend};
use crate::repo::SnapshotFile;

#[derive(Parser)]
pub(super) struct Opts {
    /// dry-run: don't restore, only show what would be done
    #[clap(long, short = 'n')]
    dry_run: bool,

    /// snapshot to restore
    id: String,

    /// restore destination
    dest: String,
}

pub(super) fn execute(be: &impl DecryptReadBackend, opts: Opts) -> Result<()> {
    println!("getting snapshot...");
    let id = Id::from_hex(&opts.id).or_else(|_| {
        // if the given id param is not a full Id, search for a suitable one
        be.find_starts_with(FileType::Snapshot, &[&opts.id])?
            .remove(0)
    })?;

    let snap = SnapshotFile::from_backend(be, &id)?;

    let dest = LocalBackend::new(&opts.dest);

    println!("reading index...");
    let index = IndexBackend::new(be)?;

    println!("1st tree walk: allocating dirs/files and collecting restore information...");
    let file_infos = allocate_and_collect(&dest, &index, snap.tree, &opts)?;

    println!("restoring file contents...");
    restore_contents(be, &dest, file_infos, &opts)?;

    println!("2nd tree walk: setting metadata");
    restore_metadata(&dest, &index, snap.tree, &opts)?;

    println!("done.");
    Ok(())
}

/// allocate files, scan or remove existing files and collect restore information
fn allocate_and_collect(
    dest: &LocalBackend,
    index: &impl IndexedBackend,
    tree: Id,
    opts: &Opts,
) -> Result<FileInfos> {
    let mut file_infos = FileInfos::new();

    // collect dirs to delete as they need to be deleted in reverse order
    //    let mut dirs_to_delete = Vec::new();

    // walk over tree in repository and compare with tree in dest
    for file in
        tree_iterator(index, vec![tree]).merge_join_by(dest.walker(), |(path, _), j| path.cmp(j))
    {
        match file {
            // node is only in snapshot
            Left((path, node)) => {
                match node.node_type() {
                    NodeType::Dir => {
                        dest.create_dir(&path);
                    }
                    NodeType::File => {
                        // collect blobs needed for restoring
                        let size = file_infos.add_file(&node, path.clone(), index);
                        // create the file
                        if !opts.dry_run {
                            dest.create_file(&path, size);
                        }
                    }
                    _ => {} // nothing to do for symlink, device, etc.
                }
            }
            // node is in snapshot but already exists
            Both((_path, _node), _file) => {}
            // node exists, but is not in snapshot
            Right(_file) => {
                /*
                if !opts.dry_run {
                    if node.is_tree() {
                        dirs_to_delete.push(file)
                    }
                    if node.is_file() {
                        dest.remove_file(&file);
                    }
                }
                */
            }
        }
    }

    /*
    for dir in dirs_to_delete.iter().rev() {
        dest.remove_dir(dir);
    }
    */

    Ok(file_infos)
}

/// restore_contents restores all files contents as described by file_infos
/// using the ReadBackend be and writing them into the LocalBackend dest.
fn restore_contents(
    be: &impl DecryptReadBackend,
    dest: &LocalBackend,
    file_infos: FileInfos,
    opts: &Opts,
) -> Result<()> {
    let (filenames, restore_info) = file_infos.dissolve();
    for (pack, blob) in restore_info {
        for (bl, fls) in blob {
            // read pack at blob_offset with length blob_length
            let data = be.read_partial(FileType::Pack, &pack, bl.offset, bl.length)?;
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

fn restore_metadata(
    dest: &LocalBackend,
    index: &impl IndexedBackend,
    tree: Id,
    opts: &Opts,
) -> Result<()> {
    // walk over tree in repository and compare with tree in dest
    for (path, node) in tree_iterator(index, vec![tree]) {
        if !opts.dry_run {
            if let NodeType::Symlink { linktarget } = node.node_type() {
                dest.create_symlink(&path, linktarget);
            }
        }
        // TODO: metadata
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
    fn add_file(&mut self, file: &Node, name: PathBuf, index: &impl IndexedBackend) -> u64 {
        let mut file_pos = 0;
        if !file.content().is_empty() {
            let file_idx = self.names.len();
            self.names.push(name);
            for id in file.content().iter() {
                let ie = index.get_id(id).unwrap();
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
        file_pos
    }
}
