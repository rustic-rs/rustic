//! `repair` index subcommand
use derive_setters::Setters;
use log::{debug, info, warn};

use std::collections::HashMap;

use crate::{
    backend::{
        decrypt::{DecryptReadBackend, DecryptWriteBackend},
        FileType, ReadBackend, WriteBackend,
    },
    error::{CommandErrorKind, RusticResult},
    index::indexer::Indexer,
    progress::{Progress, ProgressBars},
    repofile::{IndexFile, IndexPack, PackHeader, PackHeaderRef},
    repository::{Open, Repository},
};

#[cfg_attr(feature = "clap", derive(clap::Parser))]
#[derive(Default, Debug, Clone, Copy, Setters)]
#[setters(into)]
#[non_exhaustive]
/// Options for the `repair index` command
pub struct RepairIndexOptions {
    /// Read all data packs, i.e. completely re-create the index
    #[cfg_attr(feature = "clap", clap(long))]
    pub read_all: bool,
}

impl RepairIndexOptions {
    /// Runs the `repair index` command
    ///
    /// # Type Parameters
    ///
    /// * `P` - The progress bar type
    /// * `S` - The state the repository is in
    ///
    /// # Arguments
    ///
    /// * `repo` - The repository to repair
    /// * `dry_run` - Whether to actually modify the repository or just print what would be done
    pub(crate) fn repair<P: ProgressBars, S: Open>(
        self,
        repo: &Repository<P, S>,
        dry_run: bool,
    ) -> RusticResult<()> {
        let be = repo.dbe();
        let p = repo.pb.progress_spinner("listing packs...");
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

        let p = repo.pb.progress_counter("reading index...");
        for index in be.stream_all::<IndexFile>(&p)? {
            let (index_id, index) = index?;
            let mut new_index = IndexFile::default();
            let mut changed = false;
            for p in index.packs {
                process_pack(p, false, &mut new_index, &mut changed);
            }
            for p in index.packs_to_delete {
                process_pack(p, true, &mut new_index, &mut changed);
            }
            match (changed, dry_run) {
                (true, true) => info!("would have modified index file {index_id}"),
                (true, false) => {
                    if !new_index.packs.is_empty() || !new_index.packs_to_delete.is_empty() {
                        _ = be.save_file(&new_index)?;
                    }
                    be.remove(FileType::Index, &index_id, true)?;
                }
                (false, _) => {} // nothing to do
            }
        }
        p.finish();

        // process packs which are listed but not contained in the index
        pack_read_header.extend(packs.into_iter().map(|(id, size)| (id, false, None, size)));

        repo.warm_up_wait(pack_read_header.iter().map(|(id, _, _, _)| *id))?;

        let indexer = Indexer::new(be.clone()).into_shared();
        let p = repo.pb.progress_counter("reading pack headers");
        p.set_length(
            pack_read_header
                .len()
                .try_into()
                .map_err(CommandErrorKind::ConversionToU64Failed)?,
        );
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

            if !dry_run {
                indexer.write().unwrap().add_with(pack, to_delete)?;
            }
            p.inc(1);
        }
        indexer.write().unwrap().finalize()?;
        p.finish();

        Ok(())
    }
}
