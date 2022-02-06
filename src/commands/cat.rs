use anyhow::{anyhow, bail, Result};
use clap::Parser;

use crate::backend::{FileType, MapResult, ReadBackend};
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
    let tpe = match opts.tpe.as_str() {
        // special treatment for catingg blobs: read the index and use it to locate the blob
        "blob" => {
            let id = Id::from_hex(&opts.id)?;
            let dec = AllIndexFiles::new(be.clone())
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

    let id = Id::from_hex(&opts.id).or_else(|_| {
        // if the given id param is not a full Id, search for a suitable one
        let res = be.find_starts_with(tpe, &[&opts.id])?[0];
        match res {
            MapResult::Some(id) => Ok(id),
            MapResult::None => Err(anyhow!("no suitable id found for {}", &opts.id)),
            MapResult::NonUnique => Err(anyhow!("id {} is not unique", &opts.id)),
        }
    })?;

    let dec = match tpe {
        // special treatment for catting key files: those need no decryption
        FileType::Key => be.read_full(tpe, id)?,
        _ => dbe.read_full(tpe, id)?,
    };

    print!("{}", String::from_utf8_lossy(&dec));

    Ok(())
}
