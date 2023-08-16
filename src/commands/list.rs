//! `list` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{commands::open_repository, status_err, Application, RUSTIC_APP};

use abscissa_core::{Command, Runnable, Shutdown};

use anyhow::{bail, Result};

use rustic_core::repofile::{FileType, IndexFile};

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

        let repo = open_repository(&config)?;

        let tpe = match self.tpe.as_str() {
            // special treatment for listing blobs: read the index and display it
            "blobs" => {
                for item in repo.stream_files::<IndexFile>()? {
                    let (_, index) = item?;
                    index.packs.into_iter().for_each(|pack| {
                        for blob in pack.blobs {
                            println!("{:?} {:?}", blob.tpe, blob.id);
                        }
                    });
                }
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

        for id in repo.list(tpe)? {
            println!("{id:?}");
        }

        Ok(())
    }
}
