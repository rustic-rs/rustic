use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::backend::{DecryptReadBackend, FileType, ReadBackend};
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
    Key(IdOpt),
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

pub(super) fn execute(
    be: &impl ReadBackend,
    dbe: &impl DecryptReadBackend,
    opts: Opts,
) -> Result<()> {
    match opts.command {
        Command::Config(opt) => cat_file(dbe, FileType::Config, opt),
        Command::Index(opt) => cat_file(dbe, FileType::Index, opt),
        Command::Snapshot(opt) => cat_file(dbe, FileType::Snapshot, opt),
        // special treatment for catting key files: those need no decryption
        Command::Key(opt) => cat_file(be, FileType::Key, opt),
        // special treatment for catingg blobs: read the index and use it to locate the blob
        Command::TreeBlob(opt) => cat_blob(dbe, BlobType::Tree, opt),
        Command::DataBlob(opt) => cat_blob(dbe, BlobType::Data, opt),
        // special treatment for cating a tree within a snapshot
        Command::Tree(opts) => cat_tree(dbe, opts),
    }
}

fn cat_file(be: &impl ReadBackend, tpe: FileType, opt: IdOpt) -> Result<()> {
    let id = Id::from_hex(&opt.id).or_else(|_| {
        // if the given id param is not a full Id, search for a suitable one
        be.find_starts_with(tpe, &[&opt.id])?.remove(0)
    })?;
    let data = be.read_full(tpe, &id)?;
    println!("{}", String::from_utf8(data)?);

    Ok(())
}

fn cat_blob(be: &impl DecryptReadBackend, tpe: BlobType, opt: IdOpt) -> Result<()> {
    let id = Id::from_hex(&opt.id)?;
    let data = IndexBackend::new(be)?.blob_from_backend(&tpe, &id)?;
    print!("{}", String::from_utf8(data)?);

    Ok(())
}

fn cat_tree(be: &impl DecryptReadBackend, opts: TreeOpts) -> Result<()> {
    let snap = SnapshotFile::from_str(be, &opts.id, |_| true)?;
    let index = IndexBackend::new(be)?;
    let mut id = snap.tree;

    for p in opts.path.iter() {
        let p = p.to_str().unwrap();
        // TODO: check for root instead
        if p == "/" {
            continue;
        }
        let tree = Tree::from_backend(&index, &id)?;

        id = tree
            .nodes()
            .iter()
            .find(|node| node.name() == p)
            .unwrap()
            .subtree()
            .unwrap();
    }

    let data = index.blob_from_backend(&BlobType::Tree, &id)?;
    println!("{}", String::from_utf8(data)?);

    Ok(())
}
