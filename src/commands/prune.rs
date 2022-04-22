use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::str::FromStr;

use anyhow::{anyhow, bail, Result};
use bytesize::ByteSize;
use clap::Parser;
use futures::{StreamExt, TryStreamExt};
use itertools::Itertools;
use vlog::*;

use super::progress_counter;
use crate::backend::{DecryptFullBackend, DecryptReadBackend, DecryptWriteBackend, FileType};
use crate::blob::{BlobType, NodeType, Packer, TreeStreamer};
use crate::id::Id;
use crate::index::{IndexBackend, IndexedBackend, Indexer};
use crate::repo::{IndexBlob, IndexFile, IndexPack, SnapshotFile};

#[derive(Parser)]
pub(super) struct Opts {
    /// define maximum data to repack in % of reposize or as size (e.g. '5b', '2 kB', '3M', '4TiB') or 'unlimited'
    #[clap(long, value_name = "LIMIT", default_value = "unlimited")]
    max_repack: LimitOption,

    /// tolerate limit of unused data in % of reposize after pruning or as size (e.g. '5b', '2 kB', '3M', '4TiB') or 'unlimited'
    #[clap(long, value_name = "LIMIT", default_value = "5%")]
    max_unused: LimitOption,

    /// only repack packs which are cacheable
    #[clap(long)]
    repack_cacheable_only: bool,

    /// don't remove anything, only show what would be done
    #[clap(long, short = 'n')]
    dry_run: bool,
}

pub(super) async fn execute(be: &(impl DecryptFullBackend + Unpin), opts: Opts) -> Result<()> {
    let mut used_ids = {
        // TODO: in fact, we only need trees blobs and no data blobs at all here in the IndexBackend
        let indexed_be = IndexBackend::only_full_trees(be, progress_counter()).await?;
        find_used_blobs(&indexed_be).await?
    };

    v1!("reading index...");
    let mut index_files = Vec::new();

    // TODO: only read index once; was already read in IndexBackend::new
    let mut stream = be.stream_all::<IndexFile>(progress_counter()).await?;

    while let Some(index) = stream.next().await {
        index_files.push(index?)
    }

    v1!("finding duplicate blobs...");
    for pack in index_files
        .iter()
        .flat_map(|(_, index)| &index.packs)
        .unique_by(|p| p.id)
    {
        for blob in &pack.blobs {
            if let Some(count) = used_ids.get_mut(&blob.id) {
                // note that duplicates are only counted up to 255. If there are more
                // duplicates, the number is set to 255. This may imply that later on
                // not the "best" pack is chosen to have that blob marked as used.
                *count = count.saturating_add(1);
            }
        }
    }

    // list existing pack files
    v1!("geting packs from repostory...");
    let existing_packs: HashMap<_, _> = be
        .list_with_size(FileType::Pack)
        .await?
        .into_iter()
        .collect();

    let mut pruner = Pruner::new(used_ids, existing_packs);
    pruner.check()?;
    pruner.decide_packs(index_files.iter().flat_map(|(_, index)| &index.packs))?;
    pruner.decide_repack(&opts.max_repack, &opts.max_unused);
    pruner.filter_index_files(index_files);
    pruner.print_stats();

    if !opts.dry_run {
        pruner.do_prune(be).await?;
    }
    Ok(())
}

enum LimitOption {
    Size(ByteSize),
    Percentage(u64),
    Unlimited,
}

impl FromStr for LimitOption {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        Ok(match s.chars().last().unwrap_or('0') {
            '%' => Self::Percentage({
                let mut copy = s.to_string();
                copy.pop();
                copy.parse()?
            }),
            'd' if s == "unlimited" => Self::Unlimited,
            _ => Self::Size(ByteSize::from_str(s).map_err(|err| anyhow!(err))?),
        })
    }
}

#[derive(Default)]
struct PackStats {
    used: u64,
    partly_used: u64,
    unused: u64,
    keep: u64,
}
#[derive(Default)]
struct SizeStats {
    used: u64,
    unused: u64,
    remove: u64,
    repack: u64,
    repackrm: u64,
    unref: u64,
}

impl SizeStats {
    fn total(&self) -> u64 {
        self.used + self.unused
    }
    fn total_after_prune(&self) -> u64 {
        self.used + self.unused_after_prune()
    }

    fn unused_after_prune(&self) -> u64 {
        self.unused - self.remove - self.repackrm
    }
}

#[derive(Default)]
struct PruneStats {
    packs: PackStats,
    blobs: SizeStats,
    size: SizeStats,
    index_files: u64,
}

#[derive(Default)]
struct Pruner {
    used_ids: HashMap<Id, u8>,
    existing_packs: HashMap<Id, u32>,
    repack_candidates: Vec<RepackCandidate>,
    packs_remove: HashSet<Id>,
    packs_repack: HashSet<Id>,
    index_files: Vec<(Id, IndexFile)>,
    stats: PruneStats,
}

impl Pruner {
    fn new(used_ids: HashMap<Id, u8>, existing_packs: HashMap<Id, u32>) -> Self {
        Self {
            used_ids,
            existing_packs,
            ..Default::default()
        }
    }

    fn check(&self) -> Result<()> {
        // check that all used blobs are present in index
        for (id, count) in &self.used_ids {
            if *count == 0 {
                eprintln!("used blob {} is missing", id);
                bail!("missing blobs");
            }
        }
        Ok(())
    }

    fn decide_packs<'a, I>(&mut self, pack_iter: I) -> Result<()>
    where
        I: IntoIterator<Item = &'a IndexPack>,
    {
        let mut processed_packs = HashSet::new();

        // search used and unused blobs within packs
        for pack in pack_iter {
            if !processed_packs.insert(pack.id) {
                // ignore duplicate packs
                continue;
            }

            let tpe = pack.blob_type();
            let mut pi = PackInfo::new(tpe);

            // check if the pack has used blobs which are no duplicates
            let has_used = pack
                .blobs
                .iter()
                .any(|blob| self.used_ids.get(&blob.id) == Some(&1));

            for blob in &pack.blobs {
                match self.used_ids.get_mut(&blob.id) {
                    None => pi.add_unused_blob(blob),
                    Some(count) => pi.add_blob(blob, has_used, count),
                }
            }

            self.stats.blobs.used += pi.used_blobs as u64;
            self.stats.blobs.unused += pi.unused_blobs as u64;
            self.stats.size.used += pi.used_size as u64;
            self.stats.size.unused += pi.unused_size as u64;

            if pi.used_blobs == 0 {
                // unused pack
                self.stats.packs.unused += 1;
                self.packs_remove.insert(pack.id);
                self.stats.blobs.remove += pi.unused_blobs as u64;
                self.stats.size.remove += pi.unused_size as u64;

                self.existing_packs.remove(&pack.id);
            } else {
                if self.existing_packs.remove(&pack.id).is_none() {
                    bail!("used pack {} does not exist!", pack.id);
                }

                if pi.unused_blobs == 0 {
                    // used pack
                    self.stats.packs.used += 1;
                    self.stats.packs.keep += 1;
                    for blob in &pack.blobs {
                        self.used_ids.remove(&blob.id);
                    }
                } else {
                    // partly used pack => candidate for repacking
                    self.stats.packs.partly_used += 1;
                    self.repack_candidates
                        .push(RepackCandidate { id: pack.id, pi })
                }
            }
        }

        // all remaining packs in existing_packs are not needed unindexed packs
        for size in self.existing_packs.values() {
            self.stats.size.unref += *size as u64;
        }
        Ok(())
    }

    fn decide_repack(&mut self, max_repack: &LimitOption, max_unused: &LimitOption) {
        let max_unused = match max_unused {
            LimitOption::Unlimited => u64::MAX,
            LimitOption::Size(size) => size.as_u64(),
            LimitOption::Percentage(p) => (p * self.stats.size.used) / (100 - p),
        };

        let max_repack = match max_repack {
            LimitOption::Unlimited => u64::MAX,
            LimitOption::Size(size) => size.as_u64(),
            LimitOption::Percentage(p) => (p * self.stats.size.total()),
        };

        self.repack_candidates.sort_unstable_by_key(|rc| rc.pi);
        for rc in &self.repack_candidates {
            let pi = rc.pi;
            if self.stats.size.repack + (pi.unused_size + pi.used_size) as u64 >= max_repack
                || (pi.blob_type != BlobType::Tree
                    && self.stats.size.unused_after_prune() < max_unused)
            {
                self.stats.packs.keep += 1;
            } else {
                self.packs_repack.insert(rc.id);
                self.stats.blobs.repack += (pi.unused_blobs + pi.used_blobs) as u64;
                self.stats.blobs.repackrm += pi.unused_blobs as u64;
                self.stats.size.repack += (pi.unused_size + pi.used_size) as u64;
                self.stats.size.repackrm += pi.unused_size as u64;
            }
        }
    }

    fn filter_index_files(&mut self, index_files: Vec<(Id, IndexFile)>) {
        const MIN_INDEX_LEN: usize = 10_000;

        let mut any_must_modify = false;
        self.stats.index_files = index_files.len() as u64;
        let mut processed_packs = HashSet::new();
        // filter out only the index files which need processing
        self.index_files
            .extend(index_files.into_iter().filter(|(_, index)| {
                let must_modify = index.packs.iter().any(|p| {
                    // index must be processed if this is a duplicate pack
                    // or the packs needs to be removed or repacked.
                    !processed_packs.insert(p.id)
                        || self.packs_repack.contains(&p.id)
                        || self.packs_remove.contains(&p.id)
                });
                any_must_modify |= must_modify;

                must_modify || index.len() < MIN_INDEX_LEN
            }));

        if !any_must_modify && self.index_files.len() == 1 {
            // only one index file to process but only because it is too small
            self.index_files.clear();
        }
    }

    fn print_stats(&self) {
        let blob_stat = &self.stats.blobs;
        let size_stat = &self.stats.size;

        v2!(
            "used:   {:>10} blobs, {:>10}",
            blob_stat.used,
            ByteSize(size_stat.used).to_string_as(true)
        );

        v2!(
            "unused: {:>10} blobs, {:>10}",
            blob_stat.unused,
            ByteSize(size_stat.unused).to_string_as(true)
        );
        v2!(
            "total:  {:>10} blobs, {:>10}",
            blob_stat.total(),
            ByteSize(size_stat.total()).to_string_as(true)
        );

        v1!("");

        v1!(
            "to repack: {:>10} packs, {:>10} blobs, {:>10}",
            self.packs_repack.len(),
            blob_stat.repack,
            ByteSize(size_stat.repack).to_string_as(true)
        );
        v1!(
            "this removes:                {:>10} blobs, {:>10}",
            blob_stat.repackrm,
            ByteSize(size_stat.repackrm).to_string_as(true)
        );
        v1!(
            "to delete: {:>10} packs, {:>10} blobs, {:>10}",
            self.packs_remove.len(),
            blob_stat.remove,
            ByteSize(size_stat.remove).to_string_as(true)
        );
        if !self.existing_packs.is_empty() {
            v1!(
                "unindexed: {:>10} packs,         ?? blobs, {:>10}",
                self.existing_packs.len(),
                ByteSize(size_stat.unref).to_string_as(true)
            );
        }

        v1!(
            "total prune:                 {:>10} blobs, {:>10}",
            blob_stat.repackrm + blob_stat.remove,
            ByteSize(size_stat.repackrm + size_stat.remove + size_stat.unref).to_string_as(true)
        );
        v1!(
            "remaining:                   {:>10} blobs, {:>10}",
            blob_stat.total_after_prune(),
            ByteSize(size_stat.total_after_prune()).to_string_as(true)
        );
        v1!(
            "unused size after prune: {:>10} ({:.2}% of remaining size)",
            ByteSize(size_stat.unused_after_prune()).to_string_as(true),
            blob_stat.unused_after_prune() as f64 / size_stat.total_after_prune() as f64
        );

        v2!(
            "index files to rebuild: {} / {}",
            self.index_files.len(),
            self.stats.index_files
        );
    }

    async fn do_prune(mut self, be: &impl DecryptWriteBackend) -> Result<()> {
        let mut indexes_remove = Vec::new();
        let mut processed_packs = HashSet::new();
        let indexer = Rc::new(RefCell::new(Indexer::new_unindexed(be.clone())));
        let mut packer = Packer::new(be.clone(), indexer.clone())?;

        // remove unreferenced packs first
        if !self.existing_packs.is_empty() {
            v1!("removing not needed unindexed pack files...");
        }
        for id in self.existing_packs.keys() {
            be.remove(FileType::Pack, id).await?;
        }

        // process packs by index_file
        if !self.index_files.is_empty() {
            if !self.packs_repack.is_empty() {
                v1!("repacking packs and rebuilding index...");
            } else {
                v1!("rebuilding index...");
            }
        } else {
            v1!("nothing to do!");
        }

        for (index_id, index) in self.index_files {
            for pack in index.packs {
                if !processed_packs.insert(pack.id) {
                    // ignore duplicate packs
                    continue;
                }

                if self.packs_repack.contains(&pack.id) {
                    // TODO: repack in parallel
                    for blob in pack.blobs {
                        if self.used_ids.remove(&blob.id).is_none() {
                            // don't save duplicate blobs
                            continue;
                        }

                        let data = be
                            .read_partial(FileType::Pack, &pack.id, blob.offset, blob.length)
                            .await?;
                        packer.add_raw(&data, &blob.id, blob.tpe).await?;
                    }
                } else if !self.packs_remove.contains(&pack.id) {
                    // keep pack: add to new index
                    indexer.borrow_mut().add(pack).await?;
                }
                // nothing to do for packs that are going to be removed
            }
            indexes_remove.push(index_id);
        }
        packer.finalize().await?;
        indexer.borrow().finalize().await?;

        // TODO: parallelize removing
        // TODO: add progress bar
        if !self.packs_remove.is_empty() || !self.packs_repack.is_empty() {
            v1!("removing old pack files...");
        }
        for id in self.packs_remove {
            be.remove(FileType::Pack, &id).await?;
        }
        for id in self.packs_repack {
            be.remove(FileType::Pack, &id).await?;
        }

        if !indexes_remove.is_empty() {
            v1!("removing old index files...");
        }
        for id in indexes_remove {
            be.remove(FileType::Index, &id).await?;
        }

        Ok(())
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
struct PackInfo {
    blob_type: BlobType,
    used_blobs: u16,
    unused_blobs: u16,
    used_size: u32,
    unused_size: u32,
}

impl PackInfo {
    fn new(blob_type: BlobType) -> Self {
        Self {
            blob_type,
            used_blobs: 0,
            unused_blobs: 0,
            used_size: 0,
            unused_size: 0,
        }
    }
}

impl PartialOrd<PackInfo> for PackInfo {
    fn partial_cmp(&self, other: &PackInfo) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PackInfo {
    fn cmp(&self, other: &Self) -> Ordering {
        // first order by blob type such that tree packs are picked first
        self.blob_type.cmp(&other.blob_type).then(
            // then order such that packs with highest
            // ratio unused/used space are picked first.
            // This is equivalent to ordering by unused / total space.
            (other.unused_size as u64 * self.used_size as u64)
                .cmp(&(self.unused_size as u64 * other.used_size as u64)),
        )
    }
}

impl PackInfo {
    fn add_unused_blob(&mut self, blob: &IndexBlob) {
        // used duplicate exists, mark as unused
        self.unused_size += blob.length;
        self.unused_blobs += 1;
    }

    fn add_used_blob(&mut self, blob: &IndexBlob) {
        // used duplicate exists, mark as unused
        self.used_size += blob.length;
        self.used_blobs += 1;
    }

    fn add_blob(&mut self, blob: &IndexBlob, has_used: bool, count: &mut u8) {
        match count {
            0 => self.add_unused_blob(blob),
            1 => {
                // "last" occurency ->  mark as used
                self.add_used_blob(blob);
                *count = 0;
            }
            _ if has_used => {
                // other used blobs in pack ->  mark as used
                self.add_used_blob(blob);
                *count = 0;
            }
            _ => {
                // mark as unused and decrease counter
                self.add_unused_blob(blob);
                *count -= 1;
            }
        }
    }
}

struct RepackCandidate {
    id: Id,
    pi: PackInfo,
}

// find used blobs in repo
async fn find_used_blobs(index: &(impl IndexedBackend + Unpin)) -> Result<HashMap<Id, u8>> {
    v1!("reading snapshots...");

    let snap_trees: Vec<_> = index
        .be()
        .stream_all::<SnapshotFile>(progress_counter())
        .await?
        .map_ok(|(_, snap)| snap.tree)
        .try_collect()
        .await?;

    // TODO: Add progress bar here
    v1!("finding used blobs...");
    let mut ids: HashMap<_, _> = snap_trees.iter().map(|id| (*id, 0)).collect();

    let mut tree_streamer = TreeStreamer::new(index.clone(), snap_trees, true).await?;
    while let Some(item) = tree_streamer.try_next().await? {
        let node = item.1;
        match node.node_type() {
            NodeType::File => ids.extend(node.content().iter().map(|id| (*id, 0))),
            NodeType::Dir => {
                ids.insert(node.subtree().unwrap(), 0);
            }
            _ => {} // nothing to do
        }
    }

    Ok(ids)
}
