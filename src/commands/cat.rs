//! `cat` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{
    commands::{get_repository, open_repository},
    status_err, Application, RUSTIC_APP,
};

use abscissa_core::{Command, Runnable, Shutdown};

use anyhow::Result;

use rustic_core::{BlobType, FileType};

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
        let po = config.global.progress_options;

        let repo = open_repository(get_repository(&config));

        let data = match &self.cmd {
            CatSubCmd::Config => repo.cat_file(FileType::Config, "")?,
            CatSubCmd::Index(opt) => repo.cat_file(FileType::Index, &opt.id)?,
            CatSubCmd::Snapshot(opt) => repo.cat_file(FileType::Snapshot, &opt.id)?,
            // special treatment for cating blobs: read the index and use it to locate the blob
            CatSubCmd::TreeBlob(opt) => repo.to_indexed(&po)?.cat_blob(BlobType::Tree, &opt.id)?,
            CatSubCmd::DataBlob(opt) => repo.to_indexed(&po)?.cat_blob(BlobType::Data, &opt.id)?,
            // special treatment for cating a tree within a snapshot
            CatSubCmd::Tree(opt) => repo.to_indexed(&po)?.cat_tree(
                &opt.snap,
                |sn| config.snapshot_filter.matches(sn),
                &po,
            )?,
        };
        println!("{}", String::from_utf8(data.to_vec())?);

        Ok(())
    }
}
