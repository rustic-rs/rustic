use std::path::Path;

use anyhow::Result;
use clap::{Parser, Subcommand};
use indicatif::ProgressBar;

use super::progress_counter;
use crate::backend::{DecryptReadBackend, FileType};
use crate::blob::{BlobType, Tree};
use crate::id::Id;
use crate::index::{IndexBackend, IndexedBackend};
use crate::repo::SnapshotFile;

#[derive(Parser)]
pub(super) struct Opts {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    TreeBlob(IdOpt),
    DataBlob(IdOpt),
    Config,
    Index(IdOpt),
    Snapshot(IdOpt),
    /// display a tree within a snapshot
    Tree(TreeOpts),
}

#[derive(Default, Parser)]
struct IdOpt {
    /// id to cat
    id: String,
}

#[derive(Parser)]
struct TreeOpts {
    /// snapshot/path to restore
    #[clap(value_name = "SNAPSHOT[:PATH]")]
    snap: String,
}

pub(super) async fn execute(be: &impl DecryptReadBackend, opts: Opts) -> Result<()> {
    match opts.command {
        Command::Config => cat_file(be, FileType::Config, IdOpt::default()).await,
        Command::Index(opt) => cat_file(be, FileType::Index, opt).await,
        Command::Snapshot(opt) => cat_file(be, FileType::Snapshot, opt).await,
        // special treatment for catingg blobs: read the index and use it to locate the blob
        Command::TreeBlob(opt) => cat_blob(be, BlobType::Tree, opt).await,
        Command::DataBlob(opt) => cat_blob(be, BlobType::Data, opt).await,
        // special treatment for cating a tree within a snapshot
        Command::Tree(opts) => cat_tree(be, opts).await,
    }
}

async fn cat_file(be: &impl DecryptReadBackend, tpe: FileType, opt: IdOpt) -> Result<()> {
    let id = be.find_id(tpe, &opt.id).await?;
    let data = be.read_encrypted_full(tpe, &id).await?;
    println!("{}", String::from_utf8(data.to_vec())?);

    Ok(())
}

async fn cat_blob(be: &impl DecryptReadBackend, tpe: BlobType, opt: IdOpt) -> Result<()> {
    let id = Id::from_hex(&opt.id)?;
    let data = IndexBackend::new(be, ProgressBar::hidden())
        .await?
        .blob_from_backend(&tpe, &id)
        .await?;
    print!("{}", String::from_utf8(data.to_vec())?);

    Ok(())
}

async fn cat_tree(be: &impl DecryptReadBackend, opts: TreeOpts) -> Result<()> {
    let (id, path) = opts.snap.split_once(':').unwrap_or((&opts.snap, ""));
    let snap = SnapshotFile::from_str(be, id, |_| true, progress_counter()).await?;
    let index = IndexBackend::new(be, progress_counter()).await?;
    let id = Tree::subtree_id(&index, snap.tree, Path::new(path)).await?;
    let data = index.blob_from_backend(&BlobType::Tree, &id).await?;
    println!("{}", String::from_utf8(data.to_vec())?);

    Ok(())
}
