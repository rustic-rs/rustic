//! `list` subcommand

use std::num::NonZero;

use crate::{Application, RUSTIC_APP, repository::CliOpenRepo, status_err};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::{Result, bail};

use rustic_core::repofile::{IndexFile, IndexId, KeyId, PackId, SnapshotId};

/// `list` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct ListCmd {
    /// File types to list
    #[clap(value_parser=["blobs", "indexpacks", "indexcontent", "index", "packs", "snapshots", "keys"])]
    tpe: String,
}

impl Runnable for ListCmd {
    fn run(&self) {
        if let Err(err) = RUSTIC_APP
            .config()
            .repository
            .run_open(|repo| self.inner_run(repo))
        {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl ListCmd {
    fn inner_run(&self, repo: CliOpenRepo) -> Result<()> {
        match self.tpe.as_str() {
            // special treatment for listing blobs: read the index and display it
            "blobs" | "indexpacks" | "indexcontent" => {
                for item in repo.stream_files::<IndexFile>()? {
                    let (_, index) = item?;
                    for pack in index.packs {
                        match self.tpe.as_str() {
                            "blobs" => {
                                for blob in pack.blobs {
                                    println!("{:?} {:?}", blob.tpe, blob.id);
                                }
                            }
                            "indexcontent" => {
                                for blob in pack.blobs {
                                    println!(
                                        "{:?} {:?} {:?} {} {}",
                                        blob.tpe,
                                        blob.id,
                                        pack.id,
                                        blob.length,
                                        blob.uncompressed_length.map_or(0, NonZero::get)
                                    );
                                }
                            }
                            "indexpacks" => println!(
                                "{:?} {:?} {} {}",
                                pack.blob_type(),
                                pack.id,
                                pack.pack_size(),
                                pack.time.map_or_else(String::new, |time| format!(
                                    "{}",
                                    time.format("%Y-%m-%d %H:%M:%S")
                                ))
                            ),
                            t => {
                                bail!("invalid type: {}", t);
                            }
                        }
                    }
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
