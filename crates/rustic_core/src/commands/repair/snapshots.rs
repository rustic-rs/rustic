//! `repair snapshots` subcommand
use derive_setters::Setters;
use log::{info, warn};

use std::collections::{HashMap, HashSet};

use crate::{
    backend::{decrypt::DecryptWriteBackend, node::NodeType, FileType},
    blob::{packer::Packer, tree::Tree, BlobType},
    error::RusticResult,
    id::Id,
    index::{indexer::Indexer, IndexedBackend, ReadIndex},
    progress::ProgressBars,
    repofile::{SnapshotFile, StringList},
    repository::{IndexedFull, IndexedTree, Repository},
};

#[cfg_attr(feature = "clap", derive(clap::Parser))]
#[derive(Debug, Setters)]
#[setters(into)]
/// Options for the `repair snapshots` command
pub struct RepairSnapshotsOptions {
    /// Also remove defect snapshots
    ///
    /// # Warning
    ///
    /// This can result in data loss!
    #[cfg_attr(feature = "clap", clap(long))]
    pub delete: bool,

    /// Append this suffix to repaired directory or file name
    #[cfg_attr(
        feature = "clap",
        clap(long, value_name = "SUFFIX", default_value = ".repaired")
    )]
    pub suffix: String,

    /// Tag list to set on repaired snapshots (can be specified multiple times)
    #[cfg_attr(
        feature = "clap",
        clap(long, value_name = "TAG[,TAG,..]", default_value = "repaired")
    )]
    pub tag: Vec<StringList>,
}

impl Default for RepairSnapshotsOptions {
    fn default() -> Self {
        Self {
            delete: true,
            suffix: ".repaired".to_string(),
            tag: vec![StringList(vec!["repaired".to_string()])],
        }
    }
}

// TODO: add documentation
#[derive(Clone, Copy)]
enum Changed {
    This,
    SubTree,
    None,
}

impl RepairSnapshotsOptions {
    /// Runs the `repair snapshots` command
    ///
    /// # Type Parameters
    ///
    /// * `P` - The progress bar type
    /// * `S` - The type of the indexed tree.
    ///
    /// # Arguments
    ///
    /// * `repo` - The repository to repair
    /// * `snapshots` - The snapshots to repair
    /// * `dry_run` - Whether to actually modify the repository or just print what would be done
    pub(crate) fn repair<P: ProgressBars, S: IndexedFull>(
        &self,
        repo: &Repository<P, S>,
        snapshots: Vec<SnapshotFile>,
        dry_run: bool,
    ) -> RusticResult<()> {
        let be = repo.dbe();
        let config_file = repo.config();

        let mut replaced = HashMap::new();
        let mut seen = HashSet::new();
        let mut delete = Vec::new();

        let indexer = Indexer::new(be.clone()).into_shared();
        let mut packer = Packer::new(
            be.clone(),
            BlobType::Tree,
            indexer.clone(),
            config_file,
            repo.index().total_size(BlobType::Tree),
        )?;

        for mut snap in snapshots {
            let snap_id = snap.id;
            info!("processing snapshot {snap_id}");
            match self.repair_tree(
                repo.index(),
                &mut packer,
                Some(snap.tree),
                &mut replaced,
                &mut seen,
                dry_run,
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
                    if dry_run {
                        info!("would have modified snapshot {snap_id}.");
                    } else {
                        let new_id = be.save_file(&snap)?;
                        info!("saved modified snapshot as {new_id}.");
                    }
                    delete.push(snap_id);
                }
            }
        }

        if !dry_run {
            _ = packer.finalize()?;
            indexer.write().unwrap().finalize()?;
        }

        if self.delete {
            if dry_run {
                info!("would have removed {} snapshots.", delete.len());
            } else {
                be.delete_list(
                    FileType::Snapshot,
                    true,
                    delete.iter(),
                    repo.pb.progress_counter("remove defect snapshots"),
                )?;
            }
        }

        Ok(())
    }

    /// Repairs a tree
    ///
    /// # Type Parameters
    ///
    /// * `BE` - The type of the backend.
    ///
    /// # Arguments
    ///
    /// * `be` - The backend to use
    /// * `packer` - The packer to use
    /// * `id` - The id of the tree to repair
    /// * `replaced` - A map of already replaced trees
    /// * `seen` - A set of already seen trees
    /// * `dry_run` - Whether to actually modify the repository or just print what would be done
    ///
    /// # Returns
    ///
    /// A tuple containing the change status and the id of the repaired tree
    fn repair_tree<BE: DecryptWriteBackend>(
        &self,
        be: &impl IndexedBackend,
        packer: &mut Packer<BE>,
        id: Option<Id>,
        replaced: &mut HashMap<Id, (Changed, Id)>,
        seen: &mut HashSet<Id>,
        dry_run: bool,
    ) -> RusticResult<(Changed, Id)> {
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
                            let (c, tree_id) = self.repair_tree(
                                be,
                                packer,
                                node.subtree,
                                replaced,
                                seen,
                                dry_run,
                            )?;
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
                if !be.has_tree(&new_id) && !dry_run {
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
