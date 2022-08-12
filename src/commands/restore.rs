use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::Read;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Result};
use clap::Parser;
use derive_getters::Dissolve;
use futures::{stream::FuturesUnordered, TryStreamExt};
use ignore::{DirEntry, WalkBuilder};
use tokio::spawn;
use vlog::*;

use super::{bytes, progress_bytes, progress_counter};
use crate::backend::{DecryptReadBackend, FileType, LocalBackend};
use crate::blob::{Node, NodeStreamer, NodeType, Tree};
use crate::crypto::hash;
use crate::id::Id;
use crate::index::{IndexBackend, IndexedBackend};
use crate::repo::SnapshotFile;

#[derive(Parser)]
pub(super) struct Opts {
    /// dry-run: don't restore, only show what would be done
    #[clap(long, short = 'n')]
    dry_run: bool,

    /// warm up needed data pack files by only requesting them without processing
    #[clap(long, short = 'n', requires = "dry-run")]
    warm_up: bool,

    /// warm up needed data pack files by running the command with %id replaced by pack id
    #[clap(long, short = 'n', requires = "dry-run", conflicts_with = "warm-up")]
    warm_up_command: Option<String>,

    /// remove all files/dirs destination which are not contained in snapshot.
    /// Warning: Use with care, maybe first try this with --dry-run?
    #[clap(long)]
    delete: bool,

    /// use numeric ids instead of user/groug when restoring uid/gui
    #[clap(long)]
    numeric_id: bool,

    /// snapshot/path to restore
    #[clap(value_name = "SNAPSHOT[:PATH]")]
    snap: String,

    /// restore destination
    dest: String,
}

pub(super) async fn execute(be: &(impl DecryptReadBackend + Unpin), opts: Opts) -> Result<()> {
    if let Some(command) = &opts.warm_up_command {
        if !command.contains("%id") {
            bail!("warm-up command must contain %id!")
        }
        v1!("using warm-up command {command}")
    }

    let (id, path) = opts.snap.split_once(':').unwrap_or((&opts.snap, ""));
    let snap = SnapshotFile::from_str(be, id, |_| true, progress_counter()).await?;

    let index = IndexBackend::new(be, progress_counter()).await?;
    let tree = Tree::subtree_id(&index, snap.tree, Path::new(path)).await?;

    let dest = LocalBackend::new(&opts.dest);

    v1!("collecting restore information and allocating non-existing files...");
    let file_infos = allocate_and_collect(&dest, index.clone(), tree, &opts).await?;
    v1!("total restore size: {}", bytes(file_infos.total_size));
    if file_infos.matched_size > 0 {
        v1!(
            "using {} of existing file contents.",
            bytes(file_infos.matched_size)
        );
    }

    if file_infos.total_size == file_infos.matched_size {
        v1!("all file contents are fine.");
    } else if opts.warm_up {
        v1!("warming up needed data pack files...");
        warm_up(be, file_infos).await?;
    } else if opts.warm_up_command.is_some() {
        v1!("warming up needed data pack files...");
        warm_up_command(file_infos, opts.warm_up_command.as_ref().unwrap())?;
    } else if !opts.dry_run {
        v1!("restoring missing file contents...");
        restore_contents(be, &dest, file_infos).await?;
    }

    if !opts.dry_run {
        v1!("setting metadata...");
        restore_metadata(&dest, index, tree, &opts).await?;
    }

    v1!("done.");
    Ok(())
}

/// collect restore information, scan existing files and allocate non-existing files
async fn allocate_and_collect(
    dest: &LocalBackend,
    index: impl IndexedBackend + Unpin,
    tree: Id,
    opts: &Opts,
) -> Result<FileInfos> {
    let mut file_infos = FileInfos::new();
    let mut additional_existing = false;
    // Dir stack is needed to process removal of dirs AFTER the content has been processed.
    // This is the same logic as in restore_metadata -> TODO: consollidate!
    let mut dir_stack = Vec::new();

    let mut process_existing = |entry: &DirEntry| -> Result<_> {
        if entry.depth() == 0 {
            // don't process the root dir which should be existing
            return Ok(());
        }

        match (
            opts.delete,
            opts.dry_run,
            entry.file_type().unwrap().is_dir(),
        ) {
            (true, true, true) => {
                println!("would have removed the existing dir: {:?}", entry.path())
            }
            (true, true, false) => {
                println!("would have removed the existing file: {:?}", entry.path())
            }
            (true, false, true) => {
                // remove all non-parent dirs in stack
                while let Some(stackpath) = dir_stack.last() {
                    if !entry.path().starts_with(stackpath) {
                        let path = dir_stack.pop().unwrap();
                        dest.remove_dir(path)?;
                    } else {
                        break;
                    }
                }
                // push current path to the stack
                dir_stack.push(entry.path().to_path_buf());
            }
            (true, false, false) => dest.remove_file(entry.path())?,
            (false, _, _) => {
                v2!("additional entry: {:?}", entry.path());
                additional_existing = true;
            }
        }

        Ok(())
    };

    let mut process_node = |path: &PathBuf, node: &Node| -> Result<_> {
        v3!("processing {:?}", path);
        match node.node_type() {
            NodeType::Dir => {
                if !opts.dry_run {
                    dest.create_dir(path)?;
                }
            }
            NodeType::File => {
                // collect blobs needed for restoring
                if let Some(size) = file_infos.add_file(dest, node, path.clone(), &index)? {
                    if !opts.dry_run {
                        // create the file if it doesn't exist with right size
                        dest.create_file(path, size)?;
                    }
                }
            }
            _ => {} // nothing to do for symlink, device, etc.
        }
        Ok(())
    };

    let dest_path = Path::new(&opts.dest);
    let mut dst_iter = WalkBuilder::new(dest_path)
        .follow_links(false)
        .hidden(false)
        .ignore(false)
        .sort_by_file_path(Path::cmp)
        .build()
        .filter_map(Result::ok); // TODO: print out the ignored error
    let mut next_dst = dst_iter.next();

    let mut node_streamer = NodeStreamer::new(index.clone(), tree).await?;
    let mut next_node = node_streamer.try_next().await?;

    loop {
        match (&next_dst, &next_node) {
            (None, None) => break,

            (Some(dst), None) => {
                process_existing(dst)?;
                next_dst = dst_iter.next();
            }
            (Some(dst), Some((path, node))) => match dst.path().cmp(&dest_path.join(path)) {
                Ordering::Less => {
                    process_existing(dst)?;
                    next_dst = dst_iter.next();
                }
                Ordering::Equal => {
                    // process existing node
                    // TODO: This fails or behaves wrong if the type of the existing node
                    // does not match the type of the node in the snapshot!
                    process_node(path, node)?;
                    next_dst = dst_iter.next();
                    next_node = node_streamer.try_next().await?;
                }
                Ordering::Greater => {
                    process_node(path, node)?;
                    next_node = node_streamer.try_next().await?;
                }
            },
            (None, Some((path, node))) => {
                process_node(path, node)?;
                next_node = node_streamer.try_next().await?;
            }
        }
    }

    if additional_existing {
        v1!("Note: additionals entries exist in destination");
    }

    // empty dir stack and remove dirs
    for path in dir_stack.into_iter().rev() {
        dest.remove_dir(path)?;
    }

    Ok(file_infos)
}

fn warm_up_command(file_infos: FileInfos, command: &str) -> Result<()> {
    for pack in file_infos.into_packs() {
        let id = pack.to_hex();
        let actual_command = command.replace("%id", &id);
        v1!("calling {actual_command}...");
        let mut commands: Vec<_> = actual_command.split(' ').collect();
        let status = Command::new(commands[0])
            .args(&mut commands[1..])
            .status()?;
        if !status.success() {
            bail!("warm-up command was not successful for pack {id}. {status}");
        }
    }
    Ok(())
}

async fn warm_up(be: &impl DecryptReadBackend, file_infos: FileInfos) -> Result<()> {
    let packs = file_infos.into_packs();
    let mut be = be.clone();
    be.set_option("retry", "false")?;

    let p = progress_counter();
    p.set_length(packs.len() as u64);
    let mut stream = FuturesUnordered::new();

    const MAX_READER: usize = 20;
    for pack in packs {
        while stream.len() > MAX_READER {
            stream.try_next().await?;
        }

        let p = p.clone();
        let be = be.clone();
        stream.push(spawn(async move {
            // ignore errors as they are expected from the warm-up
            _ = be.read_partial(FileType::Pack, &pack, false, 0, 1).await;
            p.inc(1);
        }))
    }

    stream.try_collect().await?;
    p.finish();

    Ok(())
}

/// restore_contents restores all files contents as described by file_infos
/// using the ReadBackend be and writing them into the LocalBackend dest.
async fn restore_contents(
    be: &impl DecryptReadBackend,
    dest: &LocalBackend,
    file_infos: FileInfos,
) -> Result<()> {
    let (filenames, restore_info, total_size, matched_size) = file_infos.dissolve();

    let p = progress_bytes();
    p.set_length(total_size - matched_size);
    let mut stream = FuturesUnordered::new();

    const MAX_READER: usize = 20;
    for (pack, blob) in restore_info {
        for (bl, fls) in blob {
            let p = p.clone();
            let be = be.clone();
            let dest = dest.clone();

            let from_file = fls
                .iter()
                .find(|fl| fl.matches)
                .map(|fl| (filenames[fl.file_idx].clone(), fl.file_start));

            let name_dests: Vec<_> = fls
                .iter()
                .filter(|fl| !fl.matches)
                .map(|fl| (filenames[fl.file_idx].clone(), fl.file_start))
                .collect();

            if !name_dests.is_empty() {
                while stream.len() > MAX_READER {
                    stream.try_next().await?;
                }

                // TODO: error handling!
                stream.push(spawn(async move {
                    let data = match from_file {
                        Some((filename, start)) => {
                            // read from existing file
                            dest.read_at(filename, start, bl.data_length()).unwrap()
                        }
                        None => {
                            // read pack at blob_offset with length blob_length
                            be.read_encrypted_partial(
                                FileType::Pack,
                                &pack,
                                false,
                                bl.offset,
                                bl.length,
                                bl.uncompressed_length,
                            )
                            .await
                            .unwrap()
                        }
                    };

                    // save into needed files in parallel
                    for (name, start) in name_dests {
                        dest.write_at(&name, start, &data).unwrap();
                        p.inc(bl.data_length());
                    }
                }))
            }
        }
    }

    stream.try_collect().await?;
    p.finish();

    Ok(())
}

async fn restore_metadata(
    dest: &LocalBackend,
    index: impl IndexedBackend + Unpin,
    tree: Id,
    opts: &Opts,
) -> Result<()> {
    // walk over tree in repository and compare with tree in dest
    let mut node_streamer = NodeStreamer::new(index, tree).await?;
    let mut dir_stack = Vec::new();
    while let Some((path, node)) = node_streamer.try_next().await? {
        match node.node_type() {
            NodeType::Dir => {
                // set metadata for all non-parent paths in stack
                while let Some((stackpath, _)) = dir_stack.last() {
                    if !path.starts_with(stackpath) {
                        let (path, node) = dir_stack.pop().unwrap();
                        set_metadata(dest, &path, &node, opts);
                    } else {
                        break;
                    }
                }
                // push current path to the stack
                dir_stack.push((path, node));
            }
            _ => set_metadata(dest, &path, &node, opts),
        }
    }

    // empty dir stack and set metadata
    for (path, node) in dir_stack.into_iter().rev() {
        set_metadata(dest, &path, &node, opts);
    }

    Ok(())
}

fn set_metadata(dest: &LocalBackend, path: &PathBuf, node: &Node, opts: &Opts) {
    v3!("processing {:?}", path);
    dest.create_special(path, node)
        .unwrap_or_else(|_| eprintln!("restore {:?}: creating special file failed.", path));
    if opts.numeric_id {
        dest.set_uid_gid(path, node.meta())
            .unwrap_or_else(|_| eprintln!("restore {:?}: setting UID/GID failed.", path));
    } else {
        dest.set_user_group(path, node.meta())
            .unwrap_or_else(|_| eprintln!("restore {:?}: setting User/Group failed.", path));
    }
    dest.set_permission(path, node.meta())
        .unwrap_or_else(|_| eprintln!("restore {:?}: chmod failed.", path));
    dest.set_times(path, node.meta())
        .unwrap_or_else(|_| eprintln!("restore {:?}: setting file times failed.", path));
}

/// struct that contains information of file contents grouped by
/// 1) pack ID,
/// 2) blob within this pack
/// 3) the actual files and position of this blob within those
#[derive(Debug, Dissolve)]
struct FileInfos {
    names: Filenames,
    r: RestoreInfo,
    total_size: u64,
    matched_size: u64,
}

type RestoreInfo = HashMap<Id, HashMap<BlobLocation, Vec<FileLocation>>>;
type Filenames = Vec<PathBuf>;

#[derive(Debug, Hash, PartialEq, Eq)]
struct BlobLocation {
    offset: u32,
    length: u32,
    uncompressed_length: Option<NonZeroU32>,
}

impl BlobLocation {
    fn data_length(&self) -> u64 {
        match self.uncompressed_length {
            None => self.length - 32, // crypto overhead
            Some(length) => length.get(),
        }
        .into()
    }
}

#[derive(Debug)]
struct FileLocation {
    file_idx: usize,
    file_start: u64,
    matches: bool, //indicates that the file exists and these contents are already correct
}

impl FileInfos {
    fn new() -> Self {
        Self {
            names: Vec::new(),
            r: HashMap::new(),
            total_size: 0,
            matched_size: 0,
        }
    }

    /// Add the file to FilesInfos using index to get blob information.
    /// Returns the computed length of the file
    fn add_file(
        &mut self,
        dest: &LocalBackend,
        file: &Node,
        name: PathBuf,
        index: &impl IndexedBackend,
    ) -> Result<Option<u64>> {
        let mut open_file = dest.get_matching_file(&name, *file.meta().size());
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
                    uncompressed_length: *ie.uncompressed_length(),
                };

                let matches = match &mut open_file {
                    Some(file) => {
                        // Existing file content; check if SHA256 matches
                        let mut vec = vec![0; ie.data_length() as usize];
                        file.read_exact(&mut vec).is_ok() && id == &hash(&vec)
                    }
                    None => false,
                };
                let length = bl.data_length();
                self.total_size += length;
                if matches {
                    self.matched_size += length;
                }

                let pack = self.r.entry(*ie.pack()).or_insert_with(HashMap::new);
                let blob_location = pack.entry(bl).or_insert_with(Vec::new);
                blob_location.push(FileLocation {
                    file_idx,
                    file_start: file_pos,
                    matches,
                });

                file_pos += ie.data_length() as u64;
            }
        }

        // Tell to allocate the size only if the file does NOT exist with matching size
        Ok(open_file.is_none().then(|| file_pos))
    }

    // filter out packs which we need
    fn into_packs(self) -> Vec<Id> {
        self.r
            .into_iter()
            .filter(|(_, blob)| blob.iter().any(|(_, fls)| fls.iter().all(|fl| !fl.matches)))
            .map(|(pack, _)| pack)
            .collect()
    }
}
