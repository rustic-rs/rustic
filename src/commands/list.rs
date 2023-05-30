//! `list` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{
    commands::{get_repository, open_repository},
    status_err, Application, RUSTIC_APP,
};

use abscissa_core::{Command, Runnable, Shutdown};

use anyhow::{bail, Result};
use indicatif::ProgressBar;

use rustic_core::{DecryptReadBackend, FileType, IndexFile, ReadBackend};

/// `list` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct ListCmd {
    /// File type to list
    #[clap(value_parser=["blobs", "index", "packs", "snapshots", "keys"])]
    tpe: String,
}

impl Runnable for ListCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl ListCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();

        let repo = open_repository(get_repository(&config));

        let tpe = match self.tpe.as_str() {
            // special treatment for listing blobs: read the index and display it
            "blobs" => {
                repo.dbe
                    .stream_all::<IndexFile>(ProgressBar::hidden())?
                    .into_iter()
                    .for_each(|index| {
                        match index {
                            Ok(it) => it,
                            Err(err) => {
                                status_err!("{}", err);
                                RUSTIC_APP.shutdown(Shutdown::Crash);
                            }
                        }
                        .1
                        .packs
                        .into_iter()
                        .for_each(|pack| {
                            for blob in pack.blobs {
                                println!("{:?} {:?}", blob.tpe, blob.id);
                            }
                        });
                    });
                return Ok(());
            }
            "index" => FileType::Index,
            "packs" => FileType::Pack,
            "snapshots" => FileType::Snapshot,
            "keys" => FileType::Key,
            t => {
                bail!("invalid type: {}", t);
            }
        };

        repo.be.list(tpe)?.into_iter().for_each(|id| {
            println!("{id:?}");
        });

        Ok(())
    }
}
