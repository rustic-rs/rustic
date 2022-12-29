use std::collections::{HashMap, HashSet};

use anyhow::Result;
use clap::{AppSettings, Parser, Subcommand};
use log::*;

use crate::backend::{DecryptFullBackend, DecryptWriteBackend, FileType};
use crate::blob::{BlobType, NodeType, Packer, Tree};
use crate::id::Id;
use crate::index::{IndexBackend, IndexedBackend, Indexer, ReadIndex};
use crate::repofile::{
    ConfigFile, IndexFile, IndexPack, PackHeader, PackHeaderRef, SnapshotFile, SnapshotFilter,
    StringList,
};
use crate::repository::OpenRepository;

use super::rustic_config::RusticConfig;
use super::{progress_counter, progress_spinner, wait, warm_up, warm_up_command};

#[derive(Parser)]
pub(super) struct Opts {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Repair the repository index
    Index(IndexOpts),
    /// Repair snapshots
    Snapshots(SnapOpts),
}

#[derive(Default, Parser)]
struct IndexOpts {
    // Only show what would be repaired
    #[clap(long, short = 'n')]
    dry_run: bool,

    // Read all data packs, i.e. completely re-create the index
    #[clap(long)]
    read_all: bool,

    /// Warm up needed data pack files by only requesting them without processing
    #[clap(long)]
    warm_up: bool,

    /// Warm up needed data pack files by running the command with %id replaced by pack id
    #[clap(long, conflicts_with = "warm-up")]
    warm_up_command: Option<String>,

    /// Duration (e.g. 10m) to wait after warm up before doing the actual restore
    #[clap(long, value_name = "DURATION", conflicts_with = "dry-run")]
    warm_up_wait: Option<humantime::Duration>,
}

#[derive(Default, Parser)]
#[clap(global_setting(AppSettings::DeriveDisplayOrder))]
struct SnapOpts {
    #[clap(flatten, help_heading = "SNAPSHOT FILTER OPTIONS")]
    filter: SnapshotFilter,

    /// Only show what would be repaired
    #[clap(long, short = 'n')]
    dry_run: bool,

    /// Also remove defect snapshots - WARNING: This can result in data loss!
    #[clap(long, short = 'n')]
    delete: bool,

    /// Append this suffix to repaired directory or file name
    #[clap(long, value_name = "SUFFIX", default_value = ".repaired")]
    suffix: String,

    /// Tag list to set on repaired snapshots (can be specified multiple times)
    #[clap(long, value_name = "TAG[,TAG,..]", default_value = "repaired")]
    tag: Vec<StringList>,

    /// Snapshots to repair. If none is given, use filter to filter from all snapshots.
    #[clap(value_name = "ID")]
    ids: Vec<String>,
}

pub(super) fn execute(repo: OpenRepository, opts: Opts, config_file: RusticConfig) -> Result<()> {
    match opts.command {
        Command::Index(opt) => repair_index(&repo.dbe, opt),
        Command::Snapshots(opt) => repair_snaps(&repo.dbe, opt, config_file, &repo.config),
    }
}

fn repair_index(be: &impl DecryptFullBackend, opts: IndexOpts) -> Result<()> {
    let p = progress_spinner("listing packs...");
    let mut packs: HashMap<_, _> = be.list_with_size(FileType::Pack)?.into_iter().collect();
    p.finish();

    let mut pack_read_header = Vec::new();

    let mut process_pack = |p: IndexPack,
                            to_delete: bool,
                            new_index: &mut IndexFile,
                            changed: &mut bool| {
        let index_size = p.pack_size();
        let id = p.id;
        match packs.remove(&id) {
            None => {
                // this pack either does not exist or was already indexed in another index file => remove from index!
                *changed = true;
                debug!("removing non-existing pack {id} from index");
            }
            Some(size) if index_size != size => {
                // pack exists, but sizes do not
                pack_read_header.push((
                    id,
                    to_delete,
                    Some(PackHeaderRef::from_index_pack(&p).size()),
                    size.max(index_size),
                ));
                info!("pack {id}: size computed by index: {index_size}, actual size: {size}, will re-read header");
                *changed = true;
            }
            _ => {
                // pack in repo and index matches
                if opts.read_all {
                    pack_read_header.push((
                        id,
                        to_delete,
                        Some(PackHeaderRef::from_index_pack(&p).size()),
                        index_size,
                    ));
                    *changed = true
                } else {
                    new_index.add(p, to_delete);
                }
            }
        }
    };

    let p = progress_counter("reading index...");
    for index in be.stream_all::<IndexFile>(p.clone())? {
        let (index_id, index) = index?;
        let mut new_index = IndexFile::default();
        let mut changed = false;
        for p in index.packs {
            process_pack(p, false, &mut new_index, &mut changed);
        }
        for p in index.packs_to_delete {
            process_pack(p, true, &mut new_index, &mut changed);
        }
        match (changed, opts.dry_run) {
            (true, true) => info!("would have modified index file {index_id}"),
            (true, false) => {
                if !new_index.packs.is_empty() || !new_index.packs_to_delete.is_empty() {
                    be.save_file(&new_index)?;
                }
                be.remove(FileType::Index, &index_id, true)?;
            }
            (false, _) => {} // nothing to do
        }
    }
    p.finish();

    // process packs which are listed but not contained in the index
    pack_read_header.extend(packs.into_iter().map(|(id, size)| (id, false, None, size)));

    if opts.warm_up {
        warm_up(be, pack_read_header.iter().map(|(id, _, _, _)| *id))?;
        if opts.dry_run {
            return Ok(());
        }
    } else if opts.warm_up_command.is_some() {
        warm_up_command(
            pack_read_header.iter().map(|(id, _, _, _)| *id),
            opts.warm_up_command.as_ref().unwrap(),
        )?;
        if opts.dry_run {
            return Ok(());
        }
    }
    wait(opts.warm_up_wait);

    let indexer = Indexer::new(be.clone()).into_shared();
    let p = progress_counter("reading pack headers");
    p.set_length(pack_read_header.len().try_into()?);
    for (id, to_delete, size_hint, packsize) in pack_read_header {
        debug!("reading pack {id}...");
        let mut pack = IndexPack::default();
        pack.set_id(id);
        pack.blobs = PackHeader::from_file(be, id, size_hint, packsize)?.into_blobs();
        if !opts.dry_run {
            indexer.write().unwrap().add_with(pack, to_delete)?;
        }
        p.inc(1);
    }
    indexer.write().unwrap().finalize()?;
    p.finish();

    Ok(())
}

fn repair_snaps(
    be: &impl DecryptFullBackend,
    mut opts: SnapOpts,
    config_file: RusticConfig,
    config: &ConfigFile,
) -> Result<()> {
    config_file.merge_into("snapshot-filter", &mut opts.filter)?;

    let snapshots = match opts.ids.is_empty() {
        true => SnapshotFile::all_from_backend(be, &opts.filter)?,
        false => SnapshotFile::from_ids(be, &opts.ids)?,
    };

    let mut replaced = HashMap::new();
    let mut seen = HashSet::new();
    let mut delete = Vec::new();

    let index = IndexBackend::new(&be.clone(), progress_counter(""))?;
    let indexer = Indexer::new(be.clone()).into_shared();
    let mut packer = Packer::new(
        be.clone(),
        BlobType::Tree,
        indexer.clone(),
        config,
        index.total_size(&BlobType::Tree),
    )?;

    for mut snap in snapshots {
        let snap_id = snap.id;
        info!("processing snapshot {snap_id}");
        match repair_tree(
            &index,
            &mut packer,
            Some(snap.tree),
            &mut replaced,
            &mut seen,
            &opts,
        )? {
            (Changed::None, _) => {
                info!("snapshot {snap_id} is ok.");
            }
            (Changed::This, _) => {
                warn!("snapshot {snap_id}: root tree is damaged -> marking for deletion!");
                delete.push(snap_id);
            }
            (Changed::SubTree, id) => {
                // change snapshot tree
                if snap.original.is_none() {
                    snap.original = Some(snap.id);
                }
                snap.set_tags(opts.tag.clone());
                snap.tree = id;
                if opts.dry_run {
                    info!("would have modified snapshot {snap_id}.");
                } else {
                    let new_id = be.save_file(&snap)?;
                    info!("saved modified snapshot as {new_id}.");
                }
                delete.push(snap_id);
            }
        }
    }

    if !opts.dry_run {
        packer.finalize()?;
        indexer.write().unwrap().finalize()?;
    }

    if opts.delete {
        if opts.dry_run {
            info!("would have removed {} snapshots.", delete.len());
        } else {
            be.delete_list(
                FileType::Snapshot,
                true,
                delete.iter(),
                progress_counter("remove defect snapshots"),
            )?;
        }
    }

    Ok(())
}

#[derive(Clone, Copy)]
enum Changed {
    This,
    SubTree,
    None,
}

fn repair_tree<BE: DecryptWriteBackend>(
    be: &impl IndexedBackend,
    packer: &mut Packer<BE>,
    id: Option<Id>,
    replaced: &mut HashMap<Id, (Changed, Id)>,
    seen: &mut HashSet<Id>,
    opts: &SnapOpts,
) -> Result<(Changed, Id)> {
    let (tree, changed) = match id {
        None => (Tree::new(), Changed::This),
        Some(id) => {
            if seen.contains(&id) {
                return Ok((Changed::None, id));
            }
            if let Some(r) = replaced.get(&id) {
                return Ok(*r);
            }

            let (tree, mut changed) = match Tree::from_backend(be, id) {
                Ok(tree) => (tree, Changed::None),
                Err(_) => {
                    warn!("tree {id} could not be loaded.");
                    (Tree::new(), Changed::This)
                }
            };
            let mut new_tree = Tree::new();

            for mut node in tree {
                match node.node_type {
                    NodeType::File {} => {
                        let mut file_changed = false;
                        let mut new_content = Vec::new();
                        let mut new_size = 0;
                        for blob in node.content.take().unwrap() {
                            match be.get_data(&blob) {
                                Some(ie) => {
                                    new_content.push(blob);
                                    new_size += ie.data_length() as u64;
                                }
                                None => {
                                    file_changed = true;
                                }
                            }
                        }
                        if file_changed {
                            warn!("file {}: contents are missing", node.name);
                            node.name += &opts.suffix;
                            changed = Changed::SubTree;
                        } else if new_size != node.meta.size {
                            info!("file {}: corrected file size", node.name);
                            changed = Changed::SubTree;
                        }
                        node.content = Some(new_content);
                        node.meta.size = new_size;
                    }
                    NodeType::Dir {} => {
                        let (c, tree_id) =
                            repair_tree(be, packer, node.subtree, replaced, seen, opts)?;
                        match c {
                            Changed::None => {}
                            Changed::This => {
                                warn!("dir {}: tree is missing", node.name);
                                node.subtree = Some(tree_id);
                                node.name += &opts.suffix;
                                changed = Changed::SubTree;
                            }
                            Changed::SubTree => {
                                node.subtree = Some(tree_id);
                                changed = Changed::SubTree;
                            }
                        }
                    }
                    _ => {} // Other types: no check needed
                }
                new_tree.add(node);
            }
            if let Changed::None = changed {
                seen.insert(id);
            }
            (new_tree, changed)
        }
    };

    match (id, changed) {
        (None, Changed::None) => panic!("this should not happen!"),
        (Some(id), Changed::None) => Ok((Changed::None, id)),
        (_, c) => {
            // the tree has been changed => save it
            let (chunk, new_id) = tree.serialize()?;
            if !be.has_tree(&new_id) && !opts.dry_run {
                packer.add(&chunk, &new_id)?;
            }
            if let Some(id) = id {
                replaced.insert(id, (c, new_id));
            }
            Ok((c, new_id))
        }
    }
}
