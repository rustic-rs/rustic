use anyhow::{anyhow, bail, Result};
use clap::Parser;

use crate::backend::{FileType, ReadBackend};
use crate::id::Id;
use crate::index::{indexfiles::AllIndexFiles, ReadIndex};

#[derive(Parser)]
pub(super) struct Opts {
    /// file type to list
    tpe: String,

    /// file type to list
    id: String,
}

pub(super) fn execute(be: &impl ReadBackend, dbe: &impl ReadBackend, opts: Opts) -> Result<()> {
    let id = Id::from_hex(&opts.id)?;

    let tpe = match opts.tpe.as_str() {
        // special treatment for catingg blobs: read the index and use it to locate the blob
        "blob" => {
            let dec = AllIndexFiles::new(be)
                .get_id(&id)
                .ok_or(anyhow!("blob not found in index"))?
                .read_data(be)?;
            print!("{}", String::from_utf8_lossy(&dec));
            return Ok(());
        }
        "config" => FileType::Config,
        "index" => FileType::Index,
        "snapshot" => FileType::Snapshot,
        "key" => FileType::Key,
        t => bail!("invalid type: {}", t),
    };

    let dec = match tpe {
        // special treatment for catting key files: those need no decryption
        FileType::Key => be.read_full(tpe, id)?,
        _ => dbe.read_full(tpe, id)?,
    };

    print!("{}", String::from_utf8_lossy(&dec));

    Ok(())
}
