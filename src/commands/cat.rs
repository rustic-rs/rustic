//! `cat` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{
    commands::{get_repository, open_repository},
    status_err, Application, RUSTIC_APP,
};

use abscissa_core::{Command, Runnable, Shutdown};

use std::path::Path;

use anyhow::{anyhow, Result};

use indicatif::ProgressBar;

use rustic_core::{
    BlobType, DecryptReadBackend, FileType, Id, IndexBackend, IndexedBackend, SnapshotFile, Tree,
};

/// `cat` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct CatCmd {
    #[clap(subcommand)]
    cmd: CatSubCmd,
}

#[derive(clap::Subcommand, Debug)]
enum CatSubCmd {
    /// Display a tree blob
    TreeBlob(IdOpt),
    /// Display a data blob
    DataBlob(IdOpt),
    /// Display the config file
    Config,
    /// Display an index file
    Index(IdOpt),
    /// Display a snapshot file
    Snapshot(IdOpt),
    /// Display a tree within a snapshot
    Tree(TreeOpts),
}

#[derive(Default, clap::Parser, Debug)]
struct IdOpt {
    /// Id to display
    id: String,
}

#[derive(clap::Parser, Debug)]
struct TreeOpts {
    /// Snapshot/path of the tree to display
    #[clap(value_name = "SNAPSHOT[:PATH]")]
    snap: String,
}

impl Runnable for CatCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl CatCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();

        let repo = open_repository(get_repository(&config));

        let be = &repo.dbe;

        match &self.cmd {
            CatSubCmd::Config => cat_file(be, FileType::Config, &IdOpt::default()),
            CatSubCmd::Index(opt) => cat_file(be, FileType::Index, opt),
            CatSubCmd::Snapshot(opt) => cat_file(be, FileType::Snapshot, opt),
            // special treatment for catingg blobs: read the index and use it to locate the blob
            CatSubCmd::TreeBlob(opt) => cat_blob(be, BlobType::Tree, opt),
            CatSubCmd::DataBlob(opt) => cat_blob(be, BlobType::Data, opt),
            // special treatment for cating a tree within a snapshot
            CatSubCmd::Tree(opts) => cat_tree(be, opts),
        }?;

        Ok(())
    }
}

fn cat_file(be: &impl DecryptReadBackend, tpe: FileType, opt: &IdOpt) -> Result<()> {
    let id = be.find_id(tpe, &opt.id)?;
    let data = be.read_encrypted_full(tpe, &id)?;
    println!("{}", String::from_utf8(data.to_vec())?);

    Ok(())
}

fn cat_blob(be: &impl DecryptReadBackend, tpe: BlobType, opt: &IdOpt) -> Result<()> {
    let id = Id::from_hex(&opt.id)?;
    let data = IndexBackend::new(be, ProgressBar::hidden())?.blob_from_backend(tpe, &id)?;
    print!("{}", String::from_utf8(data.to_vec())?);

    Ok(())
}

fn cat_tree(be: &impl DecryptReadBackend, opts: &TreeOpts) -> Result<()> {
    let config = RUSTIC_APP.config();

    let (id, path) = opts.snap.split_once(':').unwrap_or((&opts.snap, ""));
    let snap = SnapshotFile::from_str(
        be,
        id,
        |sn| config.snapshot_filter.matches(sn),
        &config.global.progress_options.progress_counter(""),
    )?;
    let index = IndexBackend::new(be, config.global.progress_options.progress_counter(""))?;
    let node = Tree::node_from_path(&index, snap.tree, Path::new(path))?;
    let id = node.subtree.ok_or_else(|| anyhow!("{path} is no dir"))?;
    let data = index.blob_from_backend(BlobType::Tree, &id)?;
    println!("{}", String::from_utf8(data.to_vec())?);

    Ok(())
}
