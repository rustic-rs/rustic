use std::path::PathBuf;

use anyhow::{anyhow, Result};
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
    Config(IdOpt),
    Index(IdOpt),
    Snapshot(IdOpt),
    /// display a tree within a snapshot
    Tree(TreeOpts),
}

#[derive(Parser)]
struct IdOpt {
    /// id to cat
    id: String,
}

#[derive(Parser)]
struct TreeOpts {
    /// snapshot id
    id: String,

    /// path within snapshot
    path: PathBuf,
}

pub(super) async fn execute(be: &impl DecryptReadBackend, opts: Opts) -> Result<()> {
    match opts.command {
        Command::Config(opt) => cat_file(be, FileType::Config, opt).await,
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
    println!("{}", String::from_utf8(data)?);

    Ok(())
}

async fn cat_blob(be: &impl DecryptReadBackend, tpe: BlobType, opt: IdOpt) -> Result<()> {
    let id = Id::from_hex(&opt.id)?;
    let data = IndexBackend::new(be, ProgressBar::hidden())
        .await?
        .blob_from_backend(&tpe, &id)
        .await?;
    print!("{}", String::from_utf8(data)?);

    Ok(())
}

async fn cat_tree(be: &impl DecryptReadBackend, opts: TreeOpts) -> Result<()> {
    let snap = SnapshotFile::from_str(be, &opts.id, |_| true, progress_counter()).await?;
    let index = IndexBackend::new(be, progress_counter()).await?;
    let mut id = snap.tree;

    for p in opts.path.iter() {
        let p = p.to_str().unwrap();
        // TODO: check for root instead
        if p == "/" {
            continue;
        }
        let tree = Tree::from_backend(&index, id).await?;
        let node = tree
            .nodes()
            .iter()
            .find(|node| node.name() == p)
            .ok_or_else(|| anyhow!("{} not found", p))?;
        id = node.subtree().ok_or_else(|| anyhow!("{} is no dir", p))?;
    }

    let data = index.blob_from_backend(&BlobType::Tree, &id).await?;
    println!("{}", String::from_utf8(data)?);

    Ok(())
}
