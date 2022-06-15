use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use anyhow::{anyhow, bail, Result};
use bytesize::ByteSize;
use chrono::{DateTime, Duration, Local};
use clap::Parser;
use futures::{future, TryStreamExt};
use vlog::*;

use super::{bytes, progress_counter};
use crate::backend::{DecryptFullBackend, DecryptReadBackend, DecryptWriteBackend, FileType};
use crate::blob::{BlobType, NodeType, Packer, TreeStreamerOnce};
use crate::id::Id;
use crate::index::{IndexBackend, IndexCollector, IndexType, IndexedBackend, Indexer};
use crate::repo::{ConfigFile, IndexBlob, IndexFile, IndexPack, SnapshotFile};

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
    pub(crate) dry_run: bool,
}

pub(super) async fn execute(
    be: &(impl DecryptFullBackend + Unpin),
    opts: Opts,
    config_id: &Id,
    ignore_snaps: Vec<Id>,
) -> Result<()> {
    v1!("reading index...");
    let mut index_files = Vec::new();

    let config: ConfigFile = be.get_file(config_id).await?;
    let zstd = match config.version {
        1 => None,
        2 => Some(0),
        _ => bail!("config version not supported!"),
    };
    let mut be = be.clone();
    be.set_zstd(zstd);

    let p = progress_counter();
    let mut stream = be.stream_all::<IndexFile>(p.clone()).await?;
    let mut index_collector = IndexCollector::new(IndexType::OnlyTrees);

    while let Some((id, index)) = stream.try_next().await? {
        index_collector.extend(index.packs.clone());
        // we add the trees from packs_to_delete to the index such that searching for
        // used blobs doesn't abort if they are already marked for deletion
        index_collector.extend(index.packs_to_delete.clone());

        index_files.push((id, index))
    }
    p.finish();

    let used_ids = {
        let indexed_be = IndexBackend::new_from_index(&be, index_collector.into_index());
        find_used_blobs(&indexed_be, ignore_snaps).await?
    };

    // list existing pack files
    v1!("geting packs from repository...");
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
    pruner.check_existing_packs()?;
    pruner.filter_index_files();
    pruner.print_stats();

    if !opts.dry_run {
        pruner.do_prune(&be).await?;
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
    size_to_delete: DeleteStats,
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
}

impl PruneIndex {
    fn len(&self) -> usize {
        self.packs.iter().map(|p| p.blobs.len()).sum()
    }
}

#[derive(Debug, PartialEq)]
enum PackToDo {
    Undecided,
    Keep,
    Repack,
    MarkDelete,
    KeepMarked,
    Recover,
    Delete,
}

#[derive(Debug)]
struct PrunePack {
    id: Id,
    blob_type: BlobType,
    size: u32,
    delete_mark: bool,
    to_do: PackToDo,
    time: Option<DateTime<Local>>,
    blobs: Vec<IndexBlob>,
}

impl PrunePack {
    fn from_index_pack(p: IndexPack, delete_mark: bool) -> Self {
        Self {
            id: p.id,
            blob_type: p.blob_type(),
            size: p.pack_size(),
            delete_mark,
            to_do: PackToDo::Undecided,
            time: p.time,
            blobs: p.blobs,
        }
    }

    fn from_index_pack_unmarked(p: IndexPack) -> Self {
        Self::from_index_pack(p, false)
    }

    fn from_index_pack_marked(p: IndexPack) -> Self {
        Self::from_index_pack(p, true)
    }

    fn into_index_pack(self) -> IndexPack {
        IndexPack {
            id: self.id,
            time: self.time,
            size: None,
            blobs: self.blobs,
        }
    }

    fn into_index_pack_with_time(self, time: DateTime<Local>) -> IndexPack {
        IndexPack {
            id: self.id,
            time: Some(time),
            size: None,
            blobs: self.blobs,
        }
    }

    fn set_todo(&mut self, todo: PackToDo, pi: &PackInfo, stats: &mut PruneStats) {
        match todo {
            PackToDo::Undecided => panic!("not possible"),
            PackToDo::Keep => {
                stats.blobs.used += pi.used_blobs as u64;
                stats.blobs.unused += pi.unused_blobs as u64;
                stats.size.used += pi.used_size as u64;
                stats.size.unused += pi.unused_size as u64;
                stats.packs.keep += 1;
            }
            PackToDo::Repack => {
                stats.blobs.used += pi.used_blobs as u64;
                stats.blobs.unused += pi.unused_blobs as u64;
                stats.size.used += pi.used_size as u64;
                stats.size.unused += pi.unused_size as u64;
                stats.packs.repack += 1;
                stats.blobs.repack += (pi.unused_blobs + pi.used_blobs) as u64;
                stats.blobs.repackrm += pi.unused_blobs as u64;
                stats.size.repack += (pi.unused_size + pi.used_size) as u64;
                stats.size.repackrm += pi.unused_size as u64;
            }

            PackToDo::MarkDelete => {
                stats.blobs.unused += pi.unused_blobs as u64;
                stats.size.unused += pi.unused_size as u64;
                stats.blobs.remove += pi.unused_blobs as u64;
                stats.size.remove += pi.unused_size as u64;
            }
            PackToDo::Recover => {
                stats.packs_to_delete.recover += 1;
                stats.size_to_delete.recover += self.size as u64;
            }
            PackToDo::Delete => {
                stats.packs_to_delete.remove += 1;
                stats.size_to_delete.remove += self.size as u64;
            }
            PackToDo::KeepMarked => {
                stats.packs_to_delete.keep += 1;
                stats.size_to_delete.keep += self.size as u64;
            }
        }
        self.to_do = todo;
    }
}

struct Pruner {
    time: DateTime<Local>,
    used_ids: HashMap<Id, u8>,
    existing_packs: HashMap<Id, u32>,
    repack_candidates: Vec<(PackInfo, usize, usize)>,
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
        let mut index_files: Vec<_> = index_files
            .into_iter()
            .map(|(id, index)| {
                let mut modified = false;
                let mut packs: Vec<_> = index
                    .packs
                    .into_iter()
                    // filter out duplicate packs
                    .filter(|p| {
                        let no_duplicate = processed_packs.insert(p.id);
                        modified |= !no_duplicate;
                        no_duplicate
                    })
                    .map(PrunePack::from_index_pack_unmarked)
                    .collect();
                packs.extend(
                    index
                        .packs_to_delete
                        .into_iter()
                        // filter out duplicate packs
                        .filter(|p| {
                            let no_duplicate = processed_packs_delete.insert(p.id);
                            modified |= !no_duplicate;
                            no_duplicate
                        })
                        .map(PrunePack::from_index_pack_marked),
                );

                PruneIndex {
                    id,
                    modified,
                    packs,
                }
            })
            .collect();

        // filter out "normally" indexed packs from packs_to_delete
        for index in index_files.iter_mut() {
            let mut modified = false;
            index.packs.retain(|p| {
                !p.delete_mark || {
                    let duplicate = processed_packs.contains(&p.id);
                    modified |= duplicate;
                    !duplicate
                }
            });

            index.modified |= modified;
        }

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
        // first process all marked packs then the unmarked ones:
        // - first processed packs are more likely to have all blobs seen as unused
        // - if marked packs have used blob but these blobs are all present in
        //   unmarked packs, we want to perform the deletion!
        for mark_case in [true, false] {
            for (index_num, index) in self.index_files.iter_mut().enumerate() {
                for (pack_num, pack) in index
                    .packs
                    .iter_mut()
                    .enumerate()
                    .filter(|(_, p)| p.delete_mark == mark_case)
                {
                    let pi = PackInfo::from_pack(pack, &mut self.used_ids);
                    let too_young = pack.time > Some(self.time - keep_pack);

                    match (pack.delete_mark, pi.used_blobs, pi.unused_blobs) {
                        (false, 0, _) => {
                            // unused pack
                            self.stats.packs.unused += 1;
                            if too_young {
                                // keep packs which are too young
                                pack.set_todo(PackToDo::Keep, &pi, &mut self.stats);
                            } else {
                                pack.set_todo(PackToDo::MarkDelete, &pi, &mut self.stats);
                            }
                        }
                        (false, 1.., 0) => {
                            // used pack
                            self.stats.packs.used += 1;
                            pack.set_todo(PackToDo::Keep, &pi, &mut self.stats);
                        }

                        (false, 1.., 1..) => {
                            // partly used pack
                            self.stats.packs.partly_used += 1;

                            if too_young || repack_cacheable_only && !pack.blob_type.is_cacheable()
                            {
                                // keep packs which are too young and non-cacheable packs if requested
                                pack.set_todo(PackToDo::Keep, &pi, &mut self.stats);
                            } else {
                                // other partly used pack => candidate for repacking
                                self.repack_candidates.push((pi, index_num, pack_num))
                            }
                        }
                        (true, 0, _) => {
                            if self.time - pack.time.expect("packs_to_delete has no time")
                                >= keep_delete
                            {
                                pack.set_todo(PackToDo::Delete, &pi, &mut self.stats);
                            } else {
                                pack.set_todo(PackToDo::KeepMarked, &pi, &mut self.stats);
                            }
                        }
                        (true, 1.., _) => {
                            // needed blobs; mark this pack for recovery
                            pack.set_todo(PackToDo::Recover, &pi, &mut self.stats);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn decide_repack(&mut self, max_repack: &LimitOption, max_unused: &LimitOption) {
        let max_unused = match max_unused {
            LimitOption::Unlimited => u64::MAX,
            LimitOption::Size(size) => size.as_u64(),
            // if percentag is given, we want to have
            // unused <= p/100 * size_after = p/100 * (size_used + unused)
            // which equals (1 - p/100) * unused <= p/100 * size_used
            LimitOption::Percentage(p) => (p * self.stats.size.used) / (100 - p),
        };

        let max_repack = match max_repack {
            LimitOption::Unlimited => u64::MAX,
            LimitOption::Size(size) => size.as_u64(),
            LimitOption::Percentage(p) => (p * self.stats.size.total()),
        };

        self.repack_candidates.sort_unstable_by_key(|rc| rc.0);

        for (pi, index_num, pack_num) in std::mem::take(&mut self.repack_candidates) {
            let pack = &mut self.index_files[index_num].packs[pack_num];

            let repack_size_new = self.stats.size.repack + (pi.unused_size + pi.used_size) as u64;
            if repack_size_new >= max_repack
                || (pi.blob_type != BlobType::Tree
                    && self.stats.size.unused_after_prune() < max_unused)
            {
                pack.set_todo(PackToDo::Keep, &pi, &mut self.stats);
            } else {
                pack.set_todo(PackToDo::Repack, &pi, &mut self.stats);
            }
        }
        self.repack_candidates.clear();
        self.repack_candidates.shrink_to_fit();
    }

    fn check_existing_packs(&mut self) -> Result<()> {
        for pack in self.index_files.iter().flat_map(|index| &index.packs) {
            let existing_size = self.existing_packs.remove(&pack.id);

            // TODO: Unused Packs which don't exist (i.e. only existing in index)
            let check_size = || {
                match existing_size {
                    Some(size) if size == pack.size => Ok(()), // size is ok => continue
                    Some(size) => bail!(
                        "used pack {}: size does not match! Expected size: {}, real size: {}",
                        pack.id,
                        pack.size,
                        size
                    ),
                    None => bail!("used pack {} does not exist!", pack.id),
                }
            };

            match pack.to_do {
                PackToDo::Undecided => {
                    bail!("should not happen!")
                }
                PackToDo::Keep | PackToDo::Recover => {
                    for blob in &pack.blobs {
                        self.used_ids.remove(&blob.id);
                    }
                    check_size()?;
                }
                PackToDo::Repack => {
                    check_size()?;
                }
                PackToDo::MarkDelete | PackToDo::Delete | PackToDo::KeepMarked => {}
            }
        }

        self.used_ids.shrink_to_fit();
        self.existing_packs.shrink_to_fit();

        // all remaining packs in existing_packs are unreferenced packs
        for size in self.existing_packs.values() {
            self.stats.size.unref += *size as u64;
        }

        Ok(())
    }

    fn filter_index_files(&mut self) {
        const MIN_INDEX_LEN: usize = 10_000;

        let mut any_must_modify = false;
        self.stats.index_files = self.index_files.len() as u64;
        // filter out only the index files which need processing
        self.index_files.retain(|index| {
            // index must be processed if it has been modified
            // or if any pack is not kept
            let must_modify = index.modified
                || index
                    .packs
                    .iter()
                    .any(|p| p.to_do != PackToDo::Keep && p.to_do != PackToDo::KeepMarked);

            any_must_modify |= must_modify;

            // also process index files which are too small (i.e. rebuild them)
            must_modify || index.len() < MIN_INDEX_LEN
        });

        if !any_must_modify && self.index_files.len() == 1 {
            // only one index file to process but only because it is too small
            self.index_files.clear();
        }

        // TODO: Sort index files such that files with deletes come first and files with
        // repacks come at end
    }

    fn print_stats(&self) {
        let pack_stat = &self.stats.packs;
        let blob_stat = &self.stats.blobs;
        let size_stat = &self.stats.size;

        v2!(
            "used:   {:>10} blobs, {:>10}",
            blob_stat.used,
            bytes(size_stat.used)
        );

        v2!(
            "unused: {:>10} blobs, {:>10}",
            blob_stat.unused,
            bytes(size_stat.unused)
        );
        v2!(
            "total:  {:>10} blobs, {:>10}",
            blob_stat.total(),
            bytes(size_stat.total())
        );

        v1!("");

        v1!(
            "to repack: {:>10} packs, {:>10} blobs, {:>10}",
            pack_stat.repack,
            blob_stat.repack,
            bytes(size_stat.repack)
        );
        v1!(
            "this removes:                {:>10} blobs, {:>10}",
            blob_stat.repackrm,
            bytes(size_stat.repackrm)
        );
        v1!(
            "to delete: {:>10} packs, {:>10} blobs, {:>10}",
            pack_stat.unused,
            blob_stat.remove,
            bytes(size_stat.remove)
        );
        if !self.existing_packs.is_empty() {
            v1!(
                "unindexed: {:>10} packs,         ?? blobs, {:>10}",
                self.existing_packs.len(),
                bytes(size_stat.unref)
            );
        }

        v1!(
            "total prune:                 {:>10} blobs, {:>10}",
            blob_stat.repackrm + blob_stat.remove,
            bytes(size_stat.repackrm + size_stat.remove + size_stat.unref)
        );
        v1!(
            "remaining:                   {:>10} blobs, {:>10}",
            blob_stat.total_after_prune(),
            bytes(size_stat.total_after_prune())
        );
        v1!(
            "unused size after prune: {:>10} ({:.2}% of remaining size)",
            bytes(size_stat.unused_after_prune()),
            size_stat.unused_after_prune() as f64 / size_stat.total_after_prune() as f64 * 100.0
        );

        v1!("");

        v1!(
            "packs marked for deletion: {:>10}, {:>10}",
            self.stats.packs_to_delete.total(),
            bytes(self.stats.size_to_delete.total()),
        );
        v1!(
            " - complete deletion:      {:>10}, {:>10}",
            self.stats.packs_to_delete.remove,
            bytes(self.stats.size_to_delete.remove),
        );
        v1!(
            " - keep marked:            {:>10}, {:>10}",
            self.stats.packs_to_delete.keep,
            bytes(self.stats.size_to_delete.keep),
        );
        v1!(
            " - recover:                {:>10}, {:>10}",
            self.stats.packs_to_delete.recover,
            bytes(self.stats.size_to_delete.recover),
        );

        v2!("");

        v2!(
            "index files to rebuild: {} / {}",
            self.index_files.len(),
            self.stats.index_files
        );
    }

    async fn do_prune(mut self, be: &impl DecryptWriteBackend) -> Result<()> {
        let indexer = Indexer::new_unindexed(be.clone()).into_shared();
        // packer without zstd configuration; packed blobs are simply copied
        let mut tree_packer = Packer::new(be.clone(), BlobType::Tree, indexer.clone(), None)?;
        let mut data_packer = Packer::new(be.clone(), BlobType::Data, indexer.clone(), None)?;

        // mark unreferenced packs for deletion
        if !self.existing_packs.is_empty() {
            v1!("marking not needed unindexed pack files for deletion...");
            for (id, size) in self.existing_packs {
                let pack = IndexPack {
                    id,
                    size: Some(size),
                    time: Some(Local::now()),
                    blobs: Vec::new(),
                };
                indexer.write().await.add_remove(pack).await?;
            }
        }

        // process packs by index_file
        match (self.index_files.is_empty(), self.stats.packs.repack > 0) {
            (true, _) => v1!("nothing to do!"),
            (false, true) => v1!("repacking packs and rebuilding index..."),
            (false, false) => v1!("rebuilding index..."),
        }

        let mut indexes_remove = Vec::new();
        let mut packs_remove = Vec::new();

        for index in self.index_files {
            for pack in index.packs.into_iter() {
                match pack.to_do {
                    PackToDo::Undecided => bail!("pack {} got no decicion what to do", pack.id),
                    PackToDo::Keep => {
                        // keep pack: add to new index
                        let pack = pack.into_index_pack();
                        indexer.write().await.add(pack).await?;
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
                            match blob.tpe {
                                BlobType::Data => &mut data_packer,
                                BlobType::Tree => &mut tree_packer,
                            }
                            .add_raw(&data, &blob.id, blob.uncompressed_length)
                            .await?;
                        }
                        // mark original pack for removal
                        let pack = pack.into_index_pack_with_time(self.time);
                        indexer.write().await.add_remove(pack).await?;
                    }
                    PackToDo::MarkDelete => {
                        // remove pack: add to new index in section packs_to_delete
                        let pack = pack.into_index_pack_with_time(self.time);
                        indexer.write().await.add_remove(pack).await?;
                    }
                    PackToDo::KeepMarked => {
                        // keep pack: add to new index
                        let pack = pack.into_index_pack();
                        indexer.write().await.add_remove(pack).await?;
                    }
                    PackToDo::Recover => {
                        // recover pack: add to new index in section packs
                        let pack = pack.into_index_pack_with_time(self.time);
                        indexer.write().await.add(pack).await?;
                    }
                    PackToDo::Delete => {
                        // delete pack
                        packs_remove.push(pack.id)
                    }
                }
            }
            indexes_remove.push(index.id);
        }
        tree_packer.finalize().await?;
        data_packer.finalize().await?;
        indexer.write().await.finalize().await?;

        if !packs_remove.is_empty() {
            v1!("removing old pack files...");
            be.delete_list(FileType::Pack, packs_remove, progress_counter())
                .await?;
        }

        if !indexes_remove.is_empty() {
            v1!("removing old index files...");
            be.delete_list(FileType::Index, indexes_remove, progress_counter())
                .await?;
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
    fn from_pack(pack: &PrunePack, used_ids: &mut HashMap<Id, u8>) -> Self {
        let mut pi = Self {
            blob_type: pack.blob_type,
            used_blobs: 0,
            unused_blobs: 0,
            used_size: 0,
            unused_size: 0,
        };

        // check if the pack has used blobs which are no duplicates
        let needed_pack = pack
            .blobs
            .iter()
            .any(|blob| used_ids.get(&blob.id) == Some(&1));

        for blob in &pack.blobs {
            let count = used_ids.get_mut(&blob.id);
            match count {
                None | Some(0) => {
                    pi.unused_size += blob.length;
                    pi.unused_blobs += 1;
                }
                Some(count) if needed_pack => {
                    pi.used_size += blob.length;
                    pi.used_blobs += 1;
                    *count = 0;
                }
                Some(count) => {
                    // mark as unused and decrease counter
                    pi.unused_size += blob.length;
                    pi.unused_blobs += 1;
                    *count -= 1;
                }
            }
        }

        pi
    }
}

// find used blobs in repo
async fn find_used_blobs(
    index: &(impl IndexedBackend + Unpin),
    ignore_snaps: Vec<Id>,
) -> Result<HashMap<Id, u8>> {
    let ignore_snaps: HashSet<_> = ignore_snaps.into_iter().collect();
    v1!("reading snapshots...");

    let p = progress_counter();
    let snap_trees: Vec<_> = index
        .be()
        .stream_all::<SnapshotFile>(p.clone())
        .await?
        // TODO: it would even better to give ignore_snaps to the streaming function instead
        // if reading and then filtering the snapshot
        .try_filter(|(id, _)| future::ready(!ignore_snaps.contains(id)))
        .map_ok(|(_, snap)| snap.tree)
        .try_collect()
        .await?;
    p.finish();

    v1!("finding used blobs...");
    let mut ids: HashMap<_, _> = snap_trees.iter().map(|id| (*id, 0)).collect();

    let mut tree_streamer =
        TreeStreamerOnce::new(index.clone(), snap_trees, progress_counter()).await?;
    while let Some(item) = tree_streamer.try_next().await? {
        let (_, tree) = item;
        for node in tree.nodes() {
            match node.node_type() {
                NodeType::File => ids.extend(node.content().iter().map(|id| (*id, 0))),
                NodeType::Dir => {
                    ids.insert(node.subtree().unwrap(), 0);
                }
                _ => {} // nothing to do
            }
        }
    }

    Ok(ids)
}
