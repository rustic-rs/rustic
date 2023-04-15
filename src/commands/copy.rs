use std::collections::BTreeSet;

use anyhow::{bail, Result};
use clap::Parser;
use log::*;
use rayon::prelude::*;

use super::{progress_counter, table_with_titles, RusticConfig};
use crate::backend::DecryptWriteBackend;
use crate::blob::{BlobType, NodeType, Packer, TreeStreamerOnce};
use crate::index::{IndexBackend, IndexedBackend, Indexer, ReadIndex};
use crate::repofile::{Id, SnapshotFile, SnapshotFilter};
use crate::repository::{OpenRepository, Repository, RepositoryOptions};

#[derive(Parser)]
pub(super) struct Opts {
    /// Snapshots to copy. If none is given, use filter options to filter from all snapshots.
    #[clap(value_name = "ID")]
    ids: Vec<String>,

    /// Don't copy any snapshot, only show what would be done
    #[clap(long, short = 'n')]
    dry_run: bool,

    #[clap(
        flatten,
        next_help_heading = "Snapshot filter options (if no snapshot is given)"
    )]
    filter: SnapshotFilter,
}

pub(super) fn execute(
    repo: OpenRepository,
    mut opts: Opts,
    config_file: RusticConfig,
) -> Result<()> {
    config_file.merge_into("snapshot-filter", &mut opts.filter)?;

    let target_opts: Vec<RepositoryOptions> = config_file.get("copy.targets")?;
    if target_opts.is_empty() {
        bail!("no [[copy.targets]] section in config file found!");
    }

    let be = &repo.dbe;
    let mut snapshots = match opts.ids.is_empty() {
        true => SnapshotFile::all_from_backend(be, &opts.filter)?,
        false => SnapshotFile::from_ids(be, &opts.ids)?,
    };
    // sort for nicer output
    snapshots.sort_unstable();

    let be = &repo.dbe;
    let index = IndexBackend::new(be, progress_counter(""))?;

    let poly = repo.config.poly()?;

    for target_opt in target_opts {
        let repo_dest = Repository::new(target_opt)?.open()?;
        info!("copying to target {}...", repo_dest.name);
        if poly != repo_dest.config.poly()? {
            bail!("cannot copy to repository with different chunker parameter (re-chunking not implemented)!");
        }
        copy(&snapshots, index.clone(), repo_dest, &opts)?;
    }
    Ok(())
}

fn copy(
    snapshots: &[SnapshotFile],
    index: impl IndexedBackend,
    repo_dest: OpenRepository,
    opts: &Opts,
) -> Result<()> {
    let be_dest = &repo_dest.dbe;

    let snapshots = relevant_snapshots(snapshots, &repo_dest, &opts.filter)?;
    match (snapshots.len(), opts.dry_run) {
        (count, true) => {
            info!("would have copied {count} snapshots");
            return Ok(());
        }
        (0, false) => {
            info!("no snapshot to copy.");
            return Ok(());
        }
        _ => {} // continue
    }

    let snap_trees: Vec<_> = snapshots.iter().map(|sn| sn.tree).collect();

    let index_dest = IndexBackend::new(be_dest, progress_counter(""))?;
    let indexer = Indexer::new(be_dest.clone()).into_shared();

    let data_packer = Packer::new(
        be_dest.clone(),
        BlobType::Data,
        indexer.clone(),
        &repo_dest.config,
        index.total_size(BlobType::Data),
    )?;
    let tree_packer = Packer::new(
        be_dest.clone(),
        BlobType::Tree,
        indexer.clone(),
        &repo_dest.config,
        index.total_size(BlobType::Tree),
    )?;

    let p = progress_counter("copying blobs in snapshots...");

    snap_trees.par_iter().try_for_each(|id| -> Result<_> {
        trace!("copy tree blob {id}");
        if !index_dest.has_tree(id) {
            let data = index.get_tree(id).unwrap().read_data(index.be())?;
            tree_packer.add(&data, id)?;
        }
        Ok(())
    })?;

    let tree_streamer = TreeStreamerOnce::new(index.clone(), snap_trees, p)?;
    tree_streamer
        .par_bridge()
        .try_for_each(|item| -> Result<_> {
            let (_, tree) = item?;
            tree.nodes().par_iter().try_for_each(|node| {
                match node.node_type() {
                    NodeType::File => {
                        node.content
                            .par_iter()
                            .flatten()
                            .try_for_each(|id| -> Result<_> {
                                trace!("copy data blob {id}");
                                if !index_dest.has_data(id) {
                                    let data = index.get_data(id).unwrap().read_data(index.be())?;
                                    data_packer.add(&data, id)?;
                                }
                                Ok(())
                            })?;
                    }

                    NodeType::Dir => {
                        let id = node.subtree().unwrap();
                        trace!("copy tree blob {id}");
                        if !index_dest.has_tree(&id) {
                            let data = index.get_tree(&id).unwrap().read_data(index.be())?;
                            tree_packer.add(&data, &id)?;
                        }
                    }

                    _ => {} // nothing to copy
                }
                Ok(())
            })
        })?;

    data_packer.finalize()?;
    tree_packer.finalize()?;
    indexer.write().unwrap().finalize()?;

    let p = progress_counter("saving snapshots...");
    be_dest.save_list(snapshots.iter(), p)?;
    Ok(())
}

fn relevant_snapshots(
    snaps: &[SnapshotFile],
    dest_repo: &OpenRepository,
    filter: &SnapshotFilter,
) -> Result<Vec<SnapshotFile>> {
    // save snapshots in destination in BTreeSet, as we want to efficiently search within to filter out already existing snapshots before copying.
    let snapshots_dest: BTreeSet<_> = SnapshotFile::all_from_backend(&dest_repo.dbe, filter)?
        .into_iter()
        .map(remove_ids)
        .collect();
    let mut table = table_with_titles(["ID", "Time", "Host", "Label", "Tags", "Paths", "Status"]);
    let snaps = snaps
        .iter()
        .cloned()
        .map(|sn| (sn.id, remove_ids(sn)))
        .filter_map(|(id, sn)| {
            let relevant = !snapshots_dest.contains(&sn);
            let tags = sn.tags.formatln();
            let paths = sn.paths.formatln();
            let time = sn.time.format("%Y-%m-%d %H:%M:%S").to_string();
            table.add_row([
                &id.to_string(),
                &time,
                &sn.hostname,
                &sn.label,
                &tags,
                &paths,
                &(if relevant { "to copy" } else { "existing" }).to_string(),
            ]);
            relevant.then_some(sn)
        })
        .collect();
    println!("{table}");

    Ok(snaps)
}

// remove ids which are not saved by the copy command (and not compared when checking if snapshots already exist in the copy target)
fn remove_ids(mut sn: SnapshotFile) -> SnapshotFile {
    sn.id = Id::default();
    sn.parent = None;
    sn
}
