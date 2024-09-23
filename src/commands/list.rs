//! `list` subcommand

use crate::{commands::open_repository, status_err, Application, RUSTIC_APP};

use abscissa_core::{Command, Runnable, Shutdown};

use anyhow::{bail, Result};

use rustic_core::repofile::{IndexFile, IndexId, KeyId, PackId, SnapshotId};

/// `list` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct ListCmd {
    /// File types to list
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
        let repo = open_repository(&config.repository)?;

        match self.tpe.as_str() {
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
            }
            "index" => {
                for id in repo.list::<IndexId>()? {
                    println!("{id:?}");
                }
            }
            "packs" => {
                for id in repo.list::<PackId>()? {
                    println!("{id:?}");
                }
            }
            "snapshots" => {
                for id in repo.list::<SnapshotId>()? {
                    println!("{id:?}");
                }
            }
            "keys" => {
                for id in repo.list::<KeyId>()? {
                    println!("{id:?}");
                }
            }
            t => {
                bail!("invalid type: {}", t);
            }
        };

        Ok(())
    }
}
