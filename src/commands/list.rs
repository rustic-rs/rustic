use anyhow::{bail, Result};
use clap::Parser;
use indicatif::ProgressBar;

use crate::backend::{DecryptReadBackend, FileType, ReadBackend};
use crate::repofile::IndexFile;
use crate::repository::OpenRepository;

#[derive(Parser)]
pub(super) struct Opts {
    /// File type to list
    #[clap(possible_values=["blobs", "index", "packs", "snapshots", "keys"])]
    tpe: String,
}

pub(super) fn execute(repo: OpenRepository, opts: Opts) -> Result<()> {
    let tpe = match opts.tpe.as_str() {
        // special treatment for listing blobs: read the index and display it
        "blobs" => {
            for index in repo.dbe.stream_all::<IndexFile>(ProgressBar::hidden())? {
                for pack in index?.1.packs {
                    for blob in pack.blobs {
                        println!("{:?} {:?}", blob.tpe, blob.id);
                    }
                }
            }
            return Ok(());
        }
        "index" => FileType::Index,
        "packs" => FileType::Pack,
        "snapshots" => FileType::Snapshot,
        "keys" => FileType::Key,
        t => bail!("invalid type: {}", t),
    };

    for id in repo.be.list(tpe)? {
        println!("{id:?}");
    }

    Ok(())
}
