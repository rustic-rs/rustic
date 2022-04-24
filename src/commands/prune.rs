use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::str::FromStr;

use anyhow::{anyhow, bail, Result};
use bytesize::ByteSize;
use chrono::{DateTime, Duration, Local};
use clap::Parser;
use futures::{StreamExt, TryStreamExt};
use humantime;
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

    /// minimum duration (e.g. 10m) to keep packs marked for deletion
    #[clap(long, value_name = "DURATION", default_value = "23h")]
    keep_delete: humantime::Duration,

    /// minimum duration (e.g. 90d) to keep packs before repacking or removing
    #[clap(long, value_name = "DURATION", default_value = "0d")]
    keep_pack: humantime::Duration,

    /// don't remove anything, only show what would be done
    #[clap(long, short = 'n')]
    dry_run: bool,
}

pub(super) async fn execute(be: &(impl DecryptFullBackend + Unpin), opts: Opts) -> Result<()> {
    let used_ids = {
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

    // list existing pack files
    v1!("geting packs from repostory...");
    let existing_packs: HashMap<_, _> = be
        .list_with_size(FileType::Pack)
        .await?
        .into_iter()
        .collect();

    let mut pruner = Pruner::new(used_ids, existing_packs, index_files);
    pruner.count_used_blobs();
    pruner.check()?;
    pruner.decide_packs(
        Duration::from_std(*opts.keep_pack)?,
        Duration::from_std(*opts.keep_delete)?,
        opts.repack_cacheable_only,
    )?;
    pruner.decide_repack(&opts.max_repack, &opts.max_unused);
    pruner.filter_index_files();
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
struct DeleteStats {
    remove: u64,
    recover: u64,
    keep: u64,
}

impl DeleteStats {
    fn total(&self) -> u64 {
        self.remove + self.recover + self.keep
    }
}
#[derive(Default)]
struct PackStats {
    used: u64,
    partly_used: u64,
    unused: u64, // this equals to packs-to-remove
    repack: u64,
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
    packs_to_delete: DeleteStats,
    packs: PackStats,
    blobs: SizeStats,
    size: SizeStats,
    index_files: u64,
}

#[derive(Debug)]
struct PruneIndex {
    id: Id,
    modified: bool,
    packs: Vec<PrunePack>,
    packs_to_delete: Vec<PrunePack>,
}

impl PruneIndex {
    fn len(&self) -> usize {
        self.packs.iter().map(|p| p.blobs.len()).sum()
    }
}

#[derive(Debug, PartialEq)]
enum PackToDo {
    Keep,
    Repack,
    Remove,
    Recover,
}

#[derive(Debug)]
struct PrunePack {
    id: Id,
    blob_type: BlobType,
    to_do: PackToDo,
    time: Option<DateTime<Local>>,
    blobs: Vec<IndexBlob>,
}

struct Pruner {
    time: DateTime<Local>,
    used_ids: HashMap<Id, u8>,
    existing_packs: HashMap<Id, u32>,
    repack_candidates: Vec<RepackCandidate>,
    index_files: Vec<PruneIndex>,
    stats: PruneStats,
}

impl Pruner {
    fn new(
        used_ids: HashMap<Id, u8>,
        existing_packs: HashMap<Id, u32>,
        index_files: Vec<(Id, IndexFile)>,
    ) -> Self {
        let mut processed_packs = HashSet::new();
        let mut processed_packs_delete = HashSet::new();
        let index_files = index_files
            .into_iter()
            .map(|(id, index)| {
                let mut modified = false;
                let packs = index
                    .packs
                    .into_iter()
                    // filter out duplicate packs
                    .filter(|p| {
                        let no_duplicate = processed_packs.insert(p.id);
                        modified |= !no_duplicate;
                        no_duplicate
                    })
                    .map(|p| PrunePack {
                        id: p.id,
                        blob_type: p.blob_type(),
                        to_do: PackToDo::Keep,
                        time: p.time,
                        blobs: p.blobs,
                    })
                    .collect();
                let packs_to_delete = index
                    .packs_to_delete
                    .into_iter()
                    // filter out duplicate packs
                    .filter(|p| {
                        let no_duplicate = processed_packs_delete.insert(p.id);
                        modified |= !no_duplicate;
                        no_duplicate
                    })
                    .map(|p| PrunePack {
                        id: p.id,
                        blob_type: p.blob_type(),
                        to_do: PackToDo::Keep,
                        time: p.time,
                        blobs: p.blobs,
                    })
                    .collect();

                PruneIndex {
                    id,
                    modified,
                    packs,
                    packs_to_delete,
                }
            })
            .collect();

        Self {
            time: Local::now(),
            used_ids,
            existing_packs,
            repack_candidates: Vec::new(),
            index_files,
            stats: PruneStats::default(),
        }
    }

    fn count_used_blobs(&mut self) {
        for blob in self
            .index_files
            .iter()
            .flat_map(|index| &index.packs)
            .flat_map(|pack| &pack.blobs)
        {
            if let Some(count) = self.used_ids.get_mut(&blob.id) {
                // note that duplicates are only counted up to 255. If there are more
                // duplicates, the number is set to 255. This may imply that later on
                // not the "best" pack is chosen to have that blob marked as used.
                *count = count.saturating_add(1);
            }
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

    fn decide_packs(
        &mut self,
        keep_pack: Duration,
        keep_delete: Duration,
        repack_cacheable_only: bool,
    ) -> Result<()> {
        for index in self.index_files.iter_mut() {
            // decide what to do for "normal" packs
            for pack in index.packs.iter_mut() {
                let mut pi = PackInfo::new(pack.blob_type);

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
                    self.existing_packs.remove(&pack.id);

                    self.stats.packs.unused += 1;
                    if pack.time >= Some(self.time - keep_pack) {
                        // keep packs which are too young
                        self.stats.packs.keep += 1;
                        for blob in &pack.blobs {
                            self.used_ids.remove(&blob.id);
                        }
                    } else {
                        pack.to_do = PackToDo::Remove;
                        self.stats.blobs.remove += pi.unused_blobs as u64;
                        self.stats.size.remove += pi.unused_size as u64;
                    }
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
                        if repack_cacheable_only && !pack.blob_type.is_cacheable()
                            || pack.time > Some(self.time - keep_pack)
                        {
                            // keep non-cacheable packs if requested and
                            // packs which are too young
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
            }

            // decide what to do for packs in packs_to_delete
            for pack in index.packs_to_delete.iter_mut() {
                // check if the pack has used blobs which are not refereced in the index of
                // packs which are not marked for deletion.
                if pack.blobs.iter().fold(false, |acc, blob| {
                    if let Some(count) = self.used_ids.get_mut(&blob.id) {
                        if *count == 0 {
                            *count = 1;
                            return true;
                        }
                    }
                    return acc;
                }) {
                    // if so, mark this pack for recovery
                    pack.to_do = PackToDo::Recover;
                    self.stats.packs_to_delete.recover += 1;
                } else if self.time - pack.time.expect("packs_to_delete has no time") >= keep_delete
                {
                    pack.to_do = PackToDo::Remove;
                    self.stats.packs_to_delete.remove += 1;
                } else {
                    self.stats.packs_to_delete.keep += 1;
                }

                self.existing_packs.remove(&pack.id);
            }
        }

        // all remaining packs in existing_packs are unindexed packs
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
        let mut packs_repack = HashSet::new();

        for rc in std::mem::take(&mut self.repack_candidates) {
            let pi = rc.pi;
            if self.stats.size.repack + (pi.unused_size + pi.used_size) as u64 >= max_repack
                || (pi.blob_type != BlobType::Tree
                    && self.stats.size.unused_after_prune() < max_unused)
            {
                self.stats.packs.keep += 1;
            } else {
                packs_repack.insert(rc.id);
                self.stats.packs.repack += 1;
                self.stats.blobs.repack += (pi.unused_blobs + pi.used_blobs) as u64;
                self.stats.blobs.repackrm += pi.unused_blobs as u64;
                self.stats.size.repack += (pi.unused_size + pi.used_size) as u64;
                self.stats.size.repackrm += pi.unused_size as u64;
            }
        }

        // mark packs-to-repack in index_files
        for pack in self
            .index_files
            .iter_mut()
            .flat_map(|index| index.packs.iter_mut())
        {
            if packs_repack.contains(&pack.id) {
                pack.to_do = PackToDo::Repack;
            }
        }
    }

    fn filter_index_files(&mut self) {
        const MIN_INDEX_LEN: usize = 10_000;

        let mut any_must_modify = false;
        self.stats.index_files = self.index_files.len() as u64;
        // filter out only the index files which need processing
        self.index_files = std::mem::take(&mut self.index_files)
            .into_iter()
            .filter(|index| {
                // index must be processed if it has been modified
                // or if any pack is not kept
                let must_modify = index.modified
                    || index.packs.iter().any(|p| p.to_do != PackToDo::Keep)
                    || index
                        .packs_to_delete
                        .iter()
                        .any(|p| p.to_do != PackToDo::Keep);

                any_must_modify |= must_modify;

                // also process index files which are too small (i.e. rebuild them)
                must_modify || index.len() < MIN_INDEX_LEN
            })
            .collect();

        if !any_must_modify && self.index_files.len() == 1 {
            // only one index file to process but only because it is too small
            self.index_files.clear();
        }
    }

    fn print_stats(&self) {
        let pack_stat = &self.stats.packs;
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
            pack_stat.repack,
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
            pack_stat.unused,
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

        v1!(
            "packs marked for deletion: {:>10}",
            self.stats.packs_to_delete.total(),
        );
        v1!(
            " - complete deletion:      {:>10}",
            self.stats.packs_to_delete.remove,
        );
        v1!(
            " - keep marked:            {:>10}",
            self.stats.packs_to_delete.keep,
        );
        v1!(
            " - recover:                {:>10}",
            self.stats.packs_to_delete.recover,
        );
    }

    async fn do_prune(mut self, be: &impl DecryptWriteBackend) -> Result<()> {
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
            if self.stats.packs.repack > 0 {
                v1!("repacking packs and rebuilding index...");
            } else {
                v1!("rebuilding index...");
            }
        } else {
            v1!("nothing to do!");
        }

        let mut indexes_remove = Vec::new();
        let mut packs_remove = Vec::new();

        for index in self.index_files {
            for pack in index.packs {
                match pack.to_do {
                    PackToDo::Recover => {
                        bail!("not supported!");
                    }
                    PackToDo::Repack => {
                        // TODO: repack in parallel
                        for blob in &pack.blobs {
                            if self.used_ids.remove(&blob.id).is_none() {
                                // don't save duplicate blobs
                                continue;
                            }
                            let data = be
                                .read_partial(FileType::Pack, &pack.id, blob.offset, blob.length)
                                .await?;
                            packer.add_raw(&data, &blob.id, blob.tpe).await?;
                        }
                        // mark pack for removal
                        let pack = IndexPack {
                            id: pack.id,
                            time: Some(self.time),
                            blobs: pack.blobs,
                        };
                        indexer.borrow_mut().add_remove(pack).await?;
                    }
                    PackToDo::Keep => {
                        // keep pack: add to new index
                        let pack = IndexPack {
                            id: pack.id,
                            time: pack.time,
                            blobs: pack.blobs,
                        };
                        indexer.borrow_mut().add(pack).await?;
                    }
                    PackToDo::Remove => {
                        // remove pack: add to new index in section packs_to_delete
                        let pack = IndexPack {
                            id: pack.id,
                            time: Some(self.time),
                            blobs: pack.blobs,
                        };
                        indexer.borrow_mut().add_remove(pack).await?;
                    }
                }
            }
            for pack in index.packs_to_delete {
                match pack.to_do {
                    PackToDo::Repack => {
                        bail!("not supported");
                    }
                    PackToDo::Keep => {
                        // keep pack: add to new index
                        let pack = IndexPack {
                            id: pack.id,
                            time: pack.time,
                            blobs: pack.blobs,
                        };
                        indexer.borrow_mut().add_remove(pack).await?;
                    }
                    PackToDo::Recover => {
                        // recover pack: add to new index in section packs
                        let pack = IndexPack {
                            id: pack.id,
                            time: Some(self.time),
                            blobs: pack.blobs,
                        };
                        indexer.borrow_mut().add(pack).await?;
                    }
                    PackToDo::Remove => {
                        // delete pack
                        packs_remove.push(pack.id)
                    }
                }
            }
            indexes_remove.push(index.id);
        }
        packer.finalize().await?;
        indexer.borrow().finalize().await?;

        // TODO: parallelize removing
        // TODO: add progress bar
        if !packs_remove.is_empty() {
            v1!("removing old pack files...");
        }
        for id in packs_remove {
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
