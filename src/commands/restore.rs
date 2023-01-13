use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::Read;
use std::num::NonZeroU32;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use chrono::{Local, TimeZone, Utc};
use clap::{AppSettings, Parser};
use derive_getters::Dissolve;
use ignore::{DirEntry, WalkBuilder};
use log::*;
use rayon::ThreadPoolBuilder;

use super::rustic_config::RusticConfig;
use super::{bytes, progress_bytes, progress_counter, wait, warm_up, warm_up_command};
use crate::backend::{DecryptReadBackend, FileType, LocalBackend};
use crate::blob::{Node, NodeStreamer, NodeType, Tree};
use crate::commands::helpers::progress_spinner;
use crate::crypto::hash;
use crate::id::Id;
use crate::index::{IndexBackend, IndexedBackend};
use crate::repofile::{SnapshotFile, SnapshotFilter};
use crate::repository::OpenRepository;

#[derive(Parser)]
#[clap(global_setting(AppSettings::DeriveDisplayOrder))]
pub(super) struct Opts {
    #[clap(flatten, help_heading = "SNAPSHOT FILTER OPTIONS (when using latest)")]
    filter: SnapshotFilter,

    /// Dry-run: don't restore, only show what would be done
    #[clap(long, short = 'n')]
    dry_run: bool,

    /// Remove all files/dirs in destination which are not contained in snapshot.
    /// WARNING: Use with care, maybe first try this first with --dry-run?
    #[clap(long)]
    delete: bool,

    /// Use numeric ids instead of user/group when restoring uid/gui
    #[clap(long)]
    numeric_id: bool,

    /// Warm up needed data pack files by only requesting them without processing
    #[clap(long)]
    warm_up: bool,

    /// Always read and verify existing files (don't trust correct modification time and file size)
    #[clap(long)]
    verify_existing: bool,

    /// Warm up needed data pack files by running the command with %id replaced by pack id
    #[clap(long, conflicts_with = "warm-up")]
    warm_up_command: Option<String>,

    /// Duration (e.g. 10m) to wait after warm up before doing the actual restore
    #[clap(long, value_name = "DURATION", conflicts_with = "dry-run")]
    warm_up_wait: Option<humantime::Duration>,

    /// Snapshot/path to restore
    #[clap(value_name = "SNAPSHOT[:PATH]")]
    snap: String,

    /// Restore destination
    #[clap(value_name = "DESTINATION")]
    dest: String,
}

pub(super) fn execute(
    repo: OpenRepository,
    mut opts: Opts,
    config_file: RusticConfig,
) -> Result<()> {
    let be = &repo.dbe;
    config_file.merge_into("snapshot-filter", &mut opts.filter)?;

    if let Some(command) = &opts.warm_up_command {
        if !command.contains("%id") {
            bail!("warm-up command must contain %id!");
        }
        info!("using warm-up command {command}");
    }

    let (id, path) = opts.snap.split_once(':').unwrap_or((&opts.snap, ""));
    let snap = SnapshotFile::from_str(be, id, |sn| sn.matches(&opts.filter), progress_counter(""))?;

    let index = IndexBackend::new(be, progress_counter(""))?;
    let node = Tree::node_from_path(&index, snap.tree, Path::new(path))?;

    let dest = LocalBackend::new(&opts.dest)?;

    let p = progress_spinner("collecting file information...");
    let (file_infos, stats) = allocate_and_collect(&dest, index.clone(), &node, &opts)?;
    p.finish();

    let fs = stats.file;
    println!(
        "Files:  {} to restore, {} unchanged, {} verified, {} to modify, {} additional",
        fs.restore, fs.unchanged, fs.verified, fs.modify, fs.additional
    );
    let ds = stats.dir;
    println!(
        "Dirs:   {} to restore, {} to modify, {} additional",
        ds.restore, fs.modify, ds.additional
    );

    info!("total restore size: {}", bytes(file_infos.restore_size));
    if file_infos.matched_size > 0 {
        info!(
            "using {} of existing file contents.",
            bytes(file_infos.matched_size)
        );
    }

    if file_infos.restore_size == 0 {
        info!("all file contents are fine.");
    } else {
        if opts.warm_up {
            warm_up(be, file_infos.to_packs().into_iter())?;
        } else if opts.warm_up_command.is_some() {
            warm_up_command(
                file_infos.to_packs().into_iter(),
                opts.warm_up_command.as_ref().unwrap(),
            )?;
        }
        wait(opts.warm_up_wait);
        if !opts.dry_run {
            restore_contents(be, &dest, file_infos)?;
        }
    }

    if !opts.dry_run {
        let p = progress_spinner("setting metadata...");
        restore_metadata(&dest, index, &node, &opts)?;
        p.finish();
        info!("restore done.");
    }

    Ok(())
}

#[derive(Default)]
struct FileStats {
    restore: u64,
    unchanged: u64,
    verified: u64,
    modify: u64,
    additional: u64,
}

#[derive(Default)]
struct RestoreStats {
    file: FileStats,
    dir: FileStats,
}

/// collect restore information, scan existing files and allocate non-existing files
fn allocate_and_collect(
    dest: &LocalBackend,
    index: impl IndexedBackend + Unpin,
    node: &Node,
    opts: &Opts,
) -> Result<(FileInfos, RestoreStats)> {
    let dest_path = Path::new(&opts.dest);
    let mut stats = RestoreStats::default();

    let mut file_infos = FileInfos::new();
    let mut additional_existing = false;
    let mut removed_dir = None;

    let mut process_existing = |entry: &DirEntry| -> Result<_> {
        if entry.depth() == 0 {
            // don't process the root dir which should be existing
            return Ok(());
        }

        debug!("additional {:?}", entry.path());
        if entry.file_type().unwrap().is_dir() {
            stats.dir.additional += 1;
        } else {
            stats.file.additional += 1;
        }
        match (
            opts.delete,
            opts.dry_run,
            entry.file_type().unwrap().is_dir(),
        ) {
            (true, true, true) => {
                info!("would have removed the additional dir: {:?}", entry.path());
            }
            (true, true, false) => {
                info!("would have removed the additional file: {:?}", entry.path());
            }
            (true, false, true) => {
                let path = entry.path();
                match &removed_dir {
                    Some(dir) if path.starts_with(dir) => {}
                    _ => {
                        dest.remove_dir(path)
                            .with_context(|| format!("error removing {path:?}"))?;
                        removed_dir = Some(path.to_path_buf());
                    }
                }
            }
            (true, false, false) => dest
                .remove_file(entry.path())
                .with_context(|| format!("error removing {:?}", entry.path()))?,
            (false, _, _) => {
                additional_existing = true;
            }
        }

        Ok(())
    };

    let mut process_node = |path: &PathBuf, node: &Node, exists: bool| -> Result<_> {
        match node.node_type() {
            NodeType::Dir => {
                if exists {
                    stats.dir.modify += 1;
                    trace!("existing dir {path:?}");
                } else {
                    stats.dir.restore += 1;
                    debug!("to restore: {path:?}");
                    if !opts.dry_run {
                        dest.create_dir(path)
                            .with_context(|| format!("error creating {path:?}"))?;
                    }
                }
            }
            NodeType::File => {
                // collect blobs needed for restoring
                match (
                    exists,
                    file_infos
                        .add_file(dest, node, path.clone(), &index, opts.verify_existing)
                        .with_context(|| format!("error collecting information for {path:?}"))?,
                ) {
                    // Note that exists = false and Existing or Verified can happen if the file is changed between scanning the dir
                    // and calling add_file. So we don't care about exists but trust add_file here.
                    (_, AddFileResult::Existing) => {
                        stats.file.unchanged += 1;
                        trace!("identical file: {path:?}");
                    }
                    (_, AddFileResult::Verified) => {
                        stats.file.verified += 1;
                        trace!("verified identical file: {path:?}");
                    }
                    // TODO: The differentiation between files to modify and files to create could be done only by add_file
                    // Currently, add_file never returns Modify, but always New, so we differentiate based on exists
                    (true, AddFileResult::New(size) | AddFileResult::Modify(size)) => {
                        stats.file.modify += 1;
                        debug!("to modify: {path:?}");
                        if !opts.dry_run {
                            // set the right file size
                            dest.set_length(path, size)
                                .with_context(|| format!("error setting length for {path:?}"))?;
                        }
                    }
                    (false, AddFileResult::New(size) | AddFileResult::Modify(size)) => {
                        stats.file.restore += 1;
                        debug!("to restore: {path:?}");
                        if !opts.dry_run {
                            // create the file as it doesn't exist
                            dest.set_length(path, size)
                                .with_context(|| format!("error creating {path:?}"))?;
                        }
                    }
                }
            }
            _ => {} // nothing to do for symlink, device, etc.
        }
        Ok(())
    };

    let mut dst_iter = WalkBuilder::new(dest_path)
        .follow_links(false)
        .hidden(false)
        .ignore(false)
        .sort_by_file_path(Path::cmp)
        .build()
        .filter_map(Result::ok); // TODO: print out the ignored error
    let mut next_dst = dst_iter.next();

    let mut node_streamer = NodeStreamer::new(index.clone(), node)?;
    let mut next_node = node_streamer.next().transpose()?;

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
                    if node.is_dir() != dst.file_type().unwrap().is_dir()
                        || (node.is_symlink() != dst.file_type().unwrap().is_symlink())
                    {
                        // if types do not match, first remove the existing file
                        process_existing(dst)?;
                    }
                    process_node(path, node, true)?;
                    next_dst = dst_iter.next();
                    next_node = node_streamer.next().transpose()?;
                }
                Ordering::Greater => {
                    process_node(path, node, false)?;
                    next_node = node_streamer.next().transpose()?;
                }
            },
            (None, Some((path, node))) => {
                process_node(path, node, false)?;
                next_node = node_streamer.next().transpose()?;
            }
        }
    }

    if additional_existing {
        warn!("Note: additionals entries exist in destination");
    }

    Ok((file_infos, stats))
}

/// restore_contents restores all files contents as described by file_infos
/// using the ReadBackend be and writing them into the LocalBackend dest.
fn restore_contents(
    be: &impl DecryptReadBackend,
    dest: &LocalBackend,
    file_infos: FileInfos,
) -> Result<()> {
    let (filenames, restore_info, total_size, _) = file_infos.dissolve();

    let p = progress_bytes("restoring file contents...");
    p.set_length(total_size);

    const MAX_READER: usize = 20;
    let pool = ThreadPoolBuilder::new().num_threads(MAX_READER).build()?;
    pool.in_place_scope(|s| {
        for (pack, blob) in restore_info {
            for (bl, fls) in blob {
                let from_file = fls
                    .iter()
                    .find(|fl| fl.matches)
                    .map(|fl| (filenames[fl.file_idx].clone(), fl.file_start));

                let name_dests: Vec<_> = fls
                    .iter()
                    .filter(|fl| !fl.matches)
                    .map(|fl| (filenames[fl.file_idx].clone(), fl.file_start))
                    .collect();
                let p = &p;

                if !name_dests.is_empty() {
                    // TODO: error handling!
                    s.spawn(move |s1| {
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
                                .unwrap()
                            }
                        };
                        let size = bl.data_length();

                        // save into needed files in parallel
                        for (name, start) in name_dests {
                            let data = data.clone();
                            s1.spawn(move |_| {
                                dest.write_at(&name, start, &data).unwrap();
                                p.inc(size);
                            });
                        }
                    });
                }
            }
        }
    });

    p.finish();

    Ok(())
}

fn restore_metadata(
    dest: &LocalBackend,
    index: impl IndexedBackend + Unpin,
    node: &Node,
    opts: &Opts,
) -> Result<()> {
    // walk over tree in repository and compare with tree in dest
    let mut node_streamer = NodeStreamer::new(index, node)?;
    let mut dir_stack = Vec::new();
    while let Some((path, node)) = node_streamer.next().transpose()? {
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
    debug!("setting metadata for {:?}", path);
    dest.create_special(path, node)
        .unwrap_or_else(|_| warn!("restore {:?}: creating special file failed.", path));
    if opts.numeric_id {
        dest.set_uid_gid(path, node.meta())
            .unwrap_or_else(|_| warn!("restore {:?}: setting UID/GID failed.", path));
    } else {
        dest.set_user_group(path, node.meta())
            .unwrap_or_else(|_| warn!("restore {:?}: setting User/Group failed.", path));
    }
    dest.set_permission(path, node.meta())
        .unwrap_or_else(|_| warn!("restore {:?}: chmod failed.", path));
    dest.set_times(path, node.meta())
        .unwrap_or_else(|_| warn!("restore {:?}: setting file times failed.", path));
}

/// struct that contains information of file contents grouped by
/// 1) pack ID,
/// 2) blob within this pack
/// 3) the actual files and position of this blob within those
#[derive(Debug, Dissolve)]
struct FileInfos {
    names: Filenames,
    r: RestoreInfo,
    restore_size: u64,
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

enum AddFileResult {
    Existing,
    Verified,
    New(u64),
    Modify(u64),
}

impl FileInfos {
    fn new() -> Self {
        Self {
            names: Vec::new(),
            r: HashMap::new(),
            restore_size: 0,
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
        ignore_mtime: bool,
    ) -> Result<AddFileResult> {
        let mut open_file = dest.get_matching_file(&name, *file.meta().size());
        let file_meta = file.meta();

        if !ignore_mtime {
            if let Some(meta) = open_file.as_ref().map(|f| f.metadata()).transpose()? {
                // TODO: This is the same logic as in backend/ignore.rs => consollidate!
                let mtime = Utc
                    .timestamp_opt(meta.mtime(), meta.mtime_nsec().try_into()?)
                    .single()
                    .map(|dt| dt.with_timezone(&Local));
                if meta.len() == file_meta.size && mtime == file_meta.mtime {
                    // File exists with fitting mtime => we suspect this file is ok!
                    debug!("file {name:?} exists with suitable size and mtime, accepting it!");
                    self.matched_size += file_meta.size;
                    return Ok(AddFileResult::Existing);
                }
            }
        }

        let file_idx = self.names.len();
        self.names.push(name);
        let mut file_pos = 0;
        let mut has_unmatched = false;
        for id in file.content().iter() {
            let ie = index
                .get_data(id)
                .ok_or_else(|| anyhow!("did not find id {} in index", id))?;
            let bl = BlobLocation {
                offset: *ie.offset(),
                length: *ie.length(),
                uncompressed_length: *ie.uncompressed_length(),
            };
            let length = bl.data_length();

            let matches = match &mut open_file {
                Some(file) => {
                    // Existing file content; check if SHA256 matches
                    let mut vec = vec![0; length as usize];
                    file.read_exact(&mut vec).is_ok() && id == &hash(&vec)
                }
                None => false,
            };

            let pack = self.r.entry(*ie.pack()).or_insert_with(HashMap::new);
            let blob_location = pack.entry(bl).or_insert_with(Vec::new);
            blob_location.push(FileLocation {
                file_idx,
                file_start: file_pos,
                matches,
            });

            if matches {
                self.matched_size += length;
            } else {
                self.restore_size += length;
                has_unmatched = true;
            }

            file_pos += length;
        }

        match (has_unmatched, open_file.is_some()) {
            (true, true) => Ok(AddFileResult::Modify(file_pos)),
            (true, false) => Ok(AddFileResult::New(file_pos)),
            (false, _) => Ok(AddFileResult::Verified),
        }
    }

    fn to_packs(&self) -> Vec<Id> {
        self.r
            .iter()
            // filter out packs which we need
            .filter(|(_, blob)| blob.iter().any(|(_, fls)| fls.iter().all(|fl| !fl.matches)))
            .map(|(pack, _)| *pack)
            .collect()
    }
}
