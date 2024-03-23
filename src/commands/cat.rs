//! `cat` subcommand

use crate::{
    commands::{open_repository, open_repository_indexed},
    status_err, Application, RUSTIC_APP,
};

use abscissa_core::{Command, Runnable, Shutdown};

use anyhow::Result;

use rustic_core::repofile::{BlobType, FileType};

/// `cat` subcommand
///
/// Output the contents of a file or blob
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct CatCmd {
    #[clap(subcommand)]
    cmd: CatSubCmd,
}

/// `cat` subcommands
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
        let data =
            match &self.cmd {
                CatSubCmd::Config => {
                    open_repository(&config.repository)?.cat_file(FileType::Config, "")?
                }
                CatSubCmd::Index(opt) => {
                    open_repository(&config.repository)?.cat_file(FileType::Index, &opt.id)?
                }
                CatSubCmd::Snapshot(opt) => {
                    open_repository(&config.repository)?.cat_file(FileType::Snapshot, &opt.id)?
                }
                CatSubCmd::TreeBlob(opt) => open_repository_indexed(&config.repository)?
                    .cat_blob(BlobType::Tree, &opt.id)?,
                CatSubCmd::DataBlob(opt) => open_repository_indexed(&config.repository)?
                    .cat_blob(BlobType::Data, &opt.id)?,
                CatSubCmd::Tree(opt) => open_repository_indexed(&config.repository)?
                    .cat_tree(&opt.snap, |sn| config.snapshot_filter.matches(sn))?,
            };
        println!("{}", String::from_utf8(data.to_vec())?);

        Ok(())
    }
}
