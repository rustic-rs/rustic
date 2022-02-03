use anyhow::{bail, anyhow, Result};
use clap::Parser;

use crate::backend::{FileType, ReadBackend};
use crate::index::{indexfiles::AllIndexFiles, ReadIndex};
use crate::id::Id;

#[derive(Parser)]
pub(super) struct Opts {
    /// file type to list
    tpe: String,
    
    /// file type to list
    id: String,
}

pub(super) fn execute(be: &impl ReadBackend, opts: Opts) -> Result<()> {
    let id = Id::from_hex(&opts.id)?;

    let tpe = match opts.tpe.as_str() {
        // special treatment for catingg blobs: read the index and use it to locate the blob
        "blob" => {
            let blob = AllIndexFiles::new(be).get_id(id).ok_or(anyhow!("blob not found in index"))?;
            let dec = be.read_partial(FileType::Pack, blob.pack, blob.bi.offset, blob.bi.length)?;
            print!("{}", String::from_utf8_lossy(&dec));
            return Ok(());
        }
        "config" => FileType::Config,
        "index" => FileType::Index,
        "snapshot" => FileType::Snapshot,
        "key" => FileType::Key,
        t => bail!("invalid type: {}", t),
    };

    let dec = be.read_full(tpe, id)?;
    print!("{}", String::from_utf8_lossy(&dec));

    Ok(())
}
