use anyhow::Result;
use clap::{Parser, Subcommand};
use futures::TryStreamExt;
use log::*;
use std::collections::HashMap;

use crate::backend::{DecryptFullBackend, FileType};
use crate::index::Indexer;
use crate::repo::{IndexFile, IndexPack, PackHeader, PackHeaderRef};

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

pub(super) async fn execute(be: &impl DecryptFullBackend, opts: Opts) -> Result<()> {
    match opts.command {
        Command::Index(opt) => repair_index(be, opt).await,
    }
}

async fn repair_index(be: &impl DecryptFullBackend, opts: IndexOpts) -> Result<()> {
    let p = progress_spinner("listing packs...");
    let mut packs: HashMap<_, _> = be
        .list_with_size(FileType::Pack)
        .await?
        .into_iter()
        .collect();
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
    let mut stream = be.stream_all::<IndexFile>(p.clone()).await?;
    while let Some(index) = stream.try_next().await? {
        let mut new_index = IndexFile::default();
        let mut changed = false;
        let index_id = index.0;
        let index = index.1;
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
                    be.save_file(&new_index).await?;
                }
                be.remove(FileType::Index, &index_id, true).await?;
            }
            (false, _) => {} // nothing to do
        }
    }
    p.finish();

    // process packs which are listed but not contained in the index
    pack_read_header.extend(packs.into_iter().map(|(id, size)| (id, false, None, size)));

    if opts.warm_up {
        warm_up(be, pack_read_header.iter().map(|(id, _, _, _)| *id)).await?;
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
    wait(opts.warm_up_wait).await;

    let indexer = Indexer::new(be.clone()).into_shared();
    let p = progress_counter("reading pack headers");
    p.set_length(pack_read_header.len().try_into()?);
    for (id, to_delete, size_hint, packsize) in pack_read_header {
        debug!("reading pack {id}...");
        let mut pack = IndexPack::default();
        pack.set_id(id);
        pack.blobs = PackHeader::from_file(be, id, size_hint, packsize)
            .await?
            .into_blobs();
        if !opts.dry_run {
            indexer.write().await.add_with(pack, to_delete).await?;
        }
        p.inc(1);
    }
    indexer.write().await.finalize().await?;
    p.finish();

    Ok(())
}
