//! `repair` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{
    commands::{get_repository, open_repository},
    status_err, Application, RUSTIC_APP,
};
use abscissa_core::{Command, Runnable, Shutdown};
use log::{debug, info, warn};

use std::collections::{HashMap, HashSet};

use anyhow::Result;

use rustic_core::{
    BlobType, DecryptReadBackend, DecryptWriteBackend, FileType, Id, IndexBackend, IndexFile,
    IndexPack, IndexedBackend, Indexer, NodeType, PackHeader, PackHeaderRef, Packer, ReadBackend,
    ReadIndex, SnapshotFile, StringList, Tree, WriteBackend,
};

use crate::helpers::warm_up_wait;

/// `repair` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct RepairCmd {
    #[clap(subcommand)]
    cmd: RepairSubCmd,
}

#[derive(clap::Subcommand, Debug, Runnable)]
enum RepairSubCmd {
    /// Repair the repository index
    Index(IndexSubCmd),
    /// Repair snapshots
    Snapshots(SnapSubCmd),
}

#[derive(Default, Debug, clap::Parser, Command)]
struct IndexSubCmd {
    // Read all data packs, i.e. completely re-create the index
    #[clap(long)]
    read_all: bool,
}

#[derive(Default, Debug, clap::Parser, Command)]
struct SnapSubCmd {
    /// Also remove defect snapshots - WARNING: This can result in data loss!
    #[clap(long)]
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

#[derive(Clone, Copy)]
enum Changed {
    This,
    SubTree,
    None,
}

impl Runnable for RepairCmd {
    fn run(&self) {
        self.cmd.run();
    }
}

impl Runnable for IndexSubCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl IndexSubCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();
        let progress_options = &config.global.progress_options;

        let repo = open_repository(get_repository(&config));

        let be = &repo.dbe;
        let p = progress_options.progress_spinner("listing packs...");
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
                    if self.read_all {
                        pack_read_header.push((
                            id,
                            to_delete,
                            Some(PackHeaderRef::from_index_pack(&p).size()),
                            index_size,
                        ));
                        *changed = true;
                    } else {
                        new_index.add(p, to_delete);
                    }
                }
            }
        };

        let p = progress_options.progress_counter("reading index...");
        be.stream_all::<IndexFile>(p.clone())?
            .into_iter()
            .for_each(|index| {
                let (index_id, index) = match index {
                    Ok(it) => it,
                    Err(err) => {
                        status_err!("{}", err);
                        RUSTIC_APP.shutdown(Shutdown::Crash);
                    }
                };
                let mut new_index = IndexFile::default();
                let mut changed = false;
                for p in index.packs {
                    process_pack(p, false, &mut new_index, &mut changed);
                }
                for p in index.packs_to_delete {
                    process_pack(p, true, &mut new_index, &mut changed);
                }
                match (changed, config.global.dry_run) {
                    (true, true) => info!("would have modified index file {index_id}"),
                    (true, false) => {
                        if !new_index.packs.is_empty() || !new_index.packs_to_delete.is_empty() {
                            _ = match be.save_file(&new_index) {
                                Ok(it) => it,
                                Err(err) => {
                                    status_err!("{}", err);
                                    RUSTIC_APP.shutdown(Shutdown::Crash);
                                }
                            };
                        }
                        match be.remove(FileType::Index, &index_id, true) {
                            Ok(it) => it,
                            Err(err) => {
                                status_err!("{}", err);
                                RUSTIC_APP.shutdown(Shutdown::Crash);
                            }
                        };
                    }
                    (false, _) => {} // nothing to do
                }
            });
        p.finish();

        // process packs which are listed but not contained in the index
        pack_read_header.extend(packs.into_iter().map(|(id, size)| (id, false, None, size)));

        warm_up_wait(
            &repo,
            pack_read_header.iter().map(|(id, _, _, _)| *id),
            true,
            progress_options,
        )?;

        let indexer = Indexer::new(be.clone()).into_shared();
        let p = progress_options.progress_counter("reading pack headers");
        p.set_length(pack_read_header.len().try_into()?);
        for (id, to_delete, size_hint, packsize) in pack_read_header {
            debug!("reading pack {id}...");
            let pack = IndexPack {
                id,
                blobs: match PackHeader::from_file(be, id, size_hint, packsize) {
                    Err(err) => {
                        warn!("error reading pack {id} (not processed): {err}");
                        Vec::new()
                    }
                    Ok(header) => header.into_blobs(),
                },
                ..Default::default()
            };

            if !config.global.dry_run {
                let temp = indexer.write().unwrap().add_with(pack, to_delete);
                match temp {
                    Ok(it) => it,
                    Err(err) => {
                        status_err!("{}", err);
                        RUSTIC_APP.shutdown(Shutdown::Crash);
                    }
                };
            }
            p.inc(1);
        }
        indexer.write().unwrap().finalize()?;
        p.finish();

        Ok(())
    }
}

impl Runnable for SnapSubCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl SnapSubCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();
        let progress_options = &config.global.progress_options;

        let repo = open_repository(get_repository(&config));

        let be = &repo.dbe;
        let config_file = &repo.config;

        let snapshots = if self.ids.is_empty() {
            SnapshotFile::all_from_backend(be, |sn| config.snapshot_filter.matches(sn))?
        } else {
            SnapshotFile::from_ids(be, &self.ids)?
        };

        let mut replaced = HashMap::new();
        let mut seen = HashSet::new();
        let mut delete = Vec::new();

        let index = IndexBackend::new(&be.clone(), progress_options.progress_counter(""))?;
        let indexer = Indexer::new(be.clone()).into_shared();
        let mut packer = Packer::new(
            be.clone(),
            BlobType::Tree,
            indexer.clone(),
            config_file,
            index.total_size(BlobType::Tree),
        )?;

        for mut snap in snapshots {
            let snap_id = snap.id;
            info!("processing snapshot {snap_id}");
            match self.repair_tree(
                &index,
                &mut packer,
                Some(snap.tree),
                &mut replaced,
                &mut seen,
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
                    _ = snap.set_tags(self.tag.clone());
                    snap.tree = id;
                    if config.global.dry_run {
                        info!("would have modified snapshot {snap_id}.");
                    } else {
                        let new_id = match be.save_file(&snap) {
                            Ok(it) => it,
                            Err(err) => {
                                status_err!("{}", err);
                                RUSTIC_APP.shutdown(Shutdown::Crash);
                            }
                        };
                        info!("saved modified snapshot as {new_id}.");
                    }
                    delete.push(snap_id);
                }
            }
        }

        if !config.global.dry_run {
            _ = match packer.finalize() {
                Ok(it) => it,
                Err(err) => {
                    status_err!("{}", err);
                    RUSTIC_APP.shutdown(Shutdown::Crash);
                }
            };
            let finalizer = indexer.write().unwrap().finalize();
            match finalizer {
                Ok(it) => it,
                Err(err) => {
                    status_err!("{}", err);
                    RUSTIC_APP.shutdown(Shutdown::Crash);
                }
            };
        }

        if self.delete {
            if config.global.dry_run {
                info!("would have removed {} snapshots.", delete.len());
            } else {
                be.delete_list(
                    FileType::Snapshot,
                    true,
                    delete.iter(),
                    progress_options.progress_counter("remove defect snapshots"),
                )?;
            }
        }

        Ok(())
    }

    fn repair_tree<BE: DecryptWriteBackend>(
        &self,
        be: &impl IndexedBackend,
        packer: &mut Packer<BE>,
        id: Option<Id>,
        replaced: &mut HashMap<Id, (Changed, Id)>,
        seen: &mut HashSet<Id>,
    ) -> Result<(Changed, Id)> {
        let config = RUSTIC_APP.config();

        let (tree, changed) = match id {
            None => (Tree::new(), Changed::This),
            Some(id) => {
                if seen.contains(&id) {
                    return Ok((Changed::None, id));
                }
                if let Some(r) = replaced.get(&id) {
                    return Ok(*r);
                }

                let (tree, mut changed) = Tree::from_backend(be, id).map_or_else(
                    |_err| {
                        warn!("tree {id} could not be loaded.");
                        (Tree::new(), Changed::This)
                    },
                    |tree| (tree, Changed::None),
                );

                let mut new_tree = Tree::new();

                for mut node in tree {
                    match node.node_type {
                        NodeType::File {} => {
                            let mut file_changed = false;
                            let mut new_content = Vec::new();
                            let mut new_size = 0;
                            for blob in node.content.take().unwrap() {
                                be.get_data(&blob).map_or_else(
                                    || {
                                        file_changed = true;
                                    },
                                    |ie| {
                                        new_content.push(blob);
                                        new_size += u64::from(ie.data_length());
                                    },
                                );
                            }
                            if file_changed {
                                warn!("file {}: contents are missing", node.name);
                                node.name += &self.suffix;
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
                                self.repair_tree(be, packer, node.subtree, replaced, seen)?;
                            match c {
                                Changed::None => {}
                                Changed::This => {
                                    warn!("dir {}: tree is missing", node.name);
                                    node.subtree = Some(tree_id);
                                    node.name += &self.suffix;
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
                if matches!(changed, Changed::None) {
                    _ = seen.insert(id);
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
                if !be.has_tree(&new_id) && !config.global.dry_run {
                    packer.add(chunk.into(), new_id)?;
                }
                if let Some(id) = id {
                    _ = replaced.insert(id, (c, new_id));
                }
                Ok((c, new_id))
            }
        }
    }
}
