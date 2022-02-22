use anyhow::{bail, Result};
use clap::Parser;

use crate::backend::{DecryptReadBackend, FileType};
use crate::index::AllIndexFiles;

#[derive(Parser)]
pub(super) struct Opts {
    /// file type to list
    #[clap(possible_values=["blobs", "index", "packs", "snapshots", "keys"])]
    tpe: String,
}

pub(super) fn execute(be: &impl DecryptReadBackend, opts: Opts) -> Result<()> {
    let tpe = match opts.tpe.as_str() {
        // special treatment for listing blobs: read the index and display it
        "blobs" => {
            for i in AllIndexFiles::new(be.clone()).into_iter() {
                for blob in i.blobs() {
                    println!("{:?} {}", blob.tpe(), blob.id().to_hex());
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

    for id in be.list(tpe)? {
        println!("{}", id.to_hex());
    }

    Ok(())
}
