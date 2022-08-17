use anyhow::{bail, Result};
use clap::Parser;
use futures::StreamExt;
use indicatif::ProgressBar;

use crate::backend::{DecryptReadBackend, FileType};
use crate::repo::IndexFile;

#[derive(Parser)]
pub(super) struct Opts {
    /// File type to list
    #[clap(possible_values=["blobs", "index", "packs", "snapshots", "keys"])]
    tpe: String,
}

pub(super) async fn execute(be: &impl DecryptReadBackend, opts: Opts) -> Result<()> {
    let tpe = match opts.tpe.as_str() {
        // special treatment for listing blobs: read the index and display it
        "blobs" => {
            let mut stream = be.stream_all::<IndexFile>(ProgressBar::hidden()).await?;
            while let Some(index) = stream.next().await {
                for pack in index?.1.packs {
                    for blob in pack.blobs {
                        println!("{:?} {}", blob.tpe, blob.id.to_hex());
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

    for id in be.list(tpe).await? {
        println!("{}", id.to_hex());
    }

    Ok(())
}
