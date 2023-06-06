//! `restore` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{
    commands::{get_repository, open_repository},
    status_err, Application, RUSTIC_APP,
};

use log::{debug, error, info, trace, warn};

use abscissa_core::{Command, Runnable, Shutdown};

use std::{
    cmp::Ordering,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use ignore::{DirEntry, WalkBuilder};
use rayon::ThreadPoolBuilder;

use rustic_core::{
    bytes_size_to_string, AddFileResult, DecryptReadBackend, FileInfos, FileType, IndexBackend,
    IndexedBackend, LocalDestination, Node, NodeStreamer, NodeType, RestoreStats, SnapshotFile,
    Tree, TreeStreamerOptions,
};

use crate::{filtering::SnapshotFilter, helpers::warm_up_wait};

pub(crate) mod constants {
    pub(crate) const MAX_READER_THREADS_NUM: usize = 20;
}

/// `restore` subcommand
#[allow(clippy::struct_excessive_bools)]
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct RestoreCmd {
    /// Snapshot/path to restore
    #[clap(value_name = "SNAPSHOT[:PATH]")]
    snap: String,

    /// Restore destination
    #[clap(value_name = "DESTINATION")]
    dest: String,

    /// Remove all files/dirs in destination which are not contained in snapshot.
    /// WARNING: Use with care, maybe first try this with --dry-run?
    #[clap(long)]
    delete: bool,

    /// Use numeric ids instead of user/group when restoring uid/gui
    #[clap(long)]
    numeric_id: bool,

    /// Don't restore ownership (user/group)
    #[clap(long, conflicts_with = "numeric_id")]
    no_ownership: bool,

    /// Always read and verify existing files (don't trust correct modification time and file size)
    #[clap(long)]
    verify_existing: bool,

    #[clap(flatten)]
    streamer_opts: TreeStreamerOptions,

    #[clap(
        flatten,
        next_help_heading = "Snapshot filter options (when using latest)"
    )]
    filter: SnapshotFilter,
}
impl Runnable for RestoreCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl RestoreCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();
        let progress_options = &config.global.progress_options;
        let repo = open_repository(get_repository(&config));
        let be = &repo.dbe;

        let (id, path) = self.snap.split_once(':').unwrap_or((&self.snap, ""));
        let snap = SnapshotFile::from_str(
            be,
            id,
            |sn| config.snapshot_filter.matches(sn),
            &progress_options.progress_counter(""),
        )?;

        let index = IndexBackend::new(be, progress_options.progress_counter(""))?;
        let node = Tree::node_from_path(&index, snap.tree, Path::new(path))?;

        let dest = LocalDestination::new(&self.dest, true, !node.is_dir())?;

        let p = progress_options.progress_spinner("collecting file information...");
        let (file_infos, stats) = self.allocate_and_collect(&dest, &index, &node)?;
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

        info!(
            "total restore size: {}",
            bytes_size_to_string(file_infos.restore_size)
        );
        if file_infos.matched_size > 0 {
            info!(
                "using {} of existing file contents.",
                bytes_size_to_string(file_infos.matched_size)
            );
        }

        if file_infos.restore_size == 0 {
            info!("all file contents are fine.");
        } else {
            warm_up_wait(
                &repo,
                file_infos.to_packs().into_iter(),
                !config.global.dry_run,
                progress_options,
            )?;
            if !config.global.dry_run {
                restore_contents(be, &dest, file_infos)?;
            }
        }

        if !config.global.dry_run {
            let p = progress_options.progress_spinner("setting metadata...");
            self.restore_metadata(&dest, index, &node)?;
            p.finish();
            println!("restore done.");
        }

        Ok(())
    }
}

impl RestoreCmd {
    /// collect restore information, scan existing files and allocate non-existing files
    fn allocate_and_collect<I: IndexedBackend + Unpin>(
        &self,
        dest: &LocalDestination,
        index: &I,
        node: &Node,
    ) -> Result<(FileInfos, RestoreStats)> {
        let config = RUSTIC_APP.config();
        let dest_path = Path::new(&self.dest);
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
                self.delete,
                config.global.dry_run,
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
                        _ => match dest.remove_dir(path) {
                            Ok(()) => {
                                removed_dir = Some(path.to_path_buf());
                            }
                            Err(err) => {
                                error!("error removing {path:?}: {err}");
                            }
                        },
                    }
                }
                (true, false, false) => {
                    if let Err(err) = dest.remove_file(entry.path()) {
                        error!("error removing {:?}: {err}", entry.path());
                    }
                }
                (false, _, _) => {
                    additional_existing = true;
                }
            }

            Ok(())
        };

        let mut process_node = |path: &PathBuf, node: &Node, exists: bool| -> Result<_> {
            match node.node_type {
                NodeType::Dir => {
                    if exists {
                        stats.dir.modify += 1;
                        trace!("existing dir {path:?}");
                    } else {
                        stats.dir.restore += 1;
                        debug!("to restore: {path:?}");
                        if !config.global.dry_run {
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
                            .add_file(dest, node, path.clone(), index, self.verify_existing)
                            .with_context(|| {
                                format!("error collecting information for {path:?}")
                            })?,
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
                            if !config.global.dry_run {
                                // set the right file size
                                dest.set_length(path, size).with_context(|| {
                                    format!("error setting length for {path:?}")
                                })?;
                            }
                        }
                        (false, AddFileResult::New(size) | AddFileResult::Modify(size)) => {
                            stats.file.restore += 1;
                            debug!("to restore: {path:?}");
                            if !config.global.dry_run {
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

        let mut node_streamer =
            NodeStreamer::new_with_glob(index.clone(), node, &self.streamer_opts.clone())?;
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
                        if (node.is_dir() && !dst.file_type().unwrap().is_dir())
                            || (node.is_file() && !dst.metadata().unwrap().is_file())
                            || {
                                let this = &node;
                                matches!(
                                    this.node_type,
                                    NodeType::Symlink { linktarget: _ }
                                        | NodeType::Dev { device: _ }
                                        | NodeType::Chardev { device: _ }
                                        | NodeType::Fifo
                                        | NodeType::Socket
                                )
                            }
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
            warn!("Note: additional entries exist in destination");
        }

        Ok((file_infos, stats))
    }

    fn restore_metadata(
        &self,
        dest: &LocalDestination,
        index: impl IndexedBackend + Unpin,
        node: &Node,
    ) -> Result<()> {
        // walk over tree in repository and compare with tree in dest
        let mut node_streamer =
            NodeStreamer::new_with_glob(index, node, &self.streamer_opts.clone())?;
        let mut dir_stack = Vec::new();
        while let Some((path, node)) = node_streamer.next().transpose()? {
            match node.node_type {
                NodeType::Dir => {
                    // set metadata for all non-parent paths in stack
                    while let Some((stackpath, _)) = dir_stack.last() {
                        if path.starts_with(stackpath) {
                            break;
                        }
                        let (path, node) = dir_stack.pop().unwrap();
                        self.set_metadata(dest, &path, &node);
                    }
                    // push current path to the stack
                    dir_stack.push((path, node));
                }
                _ => self.set_metadata(dest, &path, &node),
            }
        }

        // empty dir stack and set metadata
        for (path, node) in dir_stack.into_iter().rev() {
            self.set_metadata(dest, &path, &node);
        }

        Ok(())
    }

    fn set_metadata(&self, dest: &LocalDestination, path: &PathBuf, node: &Node) {
        debug!("setting metadata for {:?}", path);
        dest.create_special(path, node)
            .unwrap_or_else(|_| warn!("restore {:?}: creating special file failed.", path));
        match (self.no_ownership, self.numeric_id) {
            (true, _) => {}
            (false, true) => dest
                .set_uid_gid(path, &node.meta)
                .unwrap_or_else(|_| warn!("restore {:?}: setting UID/GID failed.", path)),
            (false, false) => dest
                .set_user_group(path, &node.meta)
                .unwrap_or_else(|_| warn!("restore {:?}: setting User/Group failed.", path)),
        }
        dest.set_permission(path, node)
            .unwrap_or_else(|_| warn!("restore {:?}: chmod failed.", path));
        dest.set_extended_attributes(path, &node.meta.extended_attributes)
            .unwrap_or_else(|_| warn!("restore {:?}: setting extended attributes failed.", path));
        dest.set_times(path, &node.meta)
            .unwrap_or_else(|_| warn!("restore {:?}: setting file times failed.", path));
    }
}

/// [`restore_contents`] restores all files contents as described by `file_infos`
/// using the [`DecryptReadBackend`] `be` and writing them into the [`LocalBackend`] `dest`.
fn restore_contents(
    be: &impl DecryptReadBackend,
    dest: &LocalDestination,
    file_infos: FileInfos,
) -> Result<()> {
    let FileInfos {
        names: filenames,
        r: restore_info,
        restore_size: total_size,
        ..
    } = file_infos;

    let p = RUSTIC_APP
        .config()
        .global
        .progress_options
        .progress_bytes("restoring file contents...");
    p.set_length(total_size);

    let pool = ThreadPoolBuilder::new()
        .num_threads(constants::MAX_READER_THREADS_NUM)
        .build()?;
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
