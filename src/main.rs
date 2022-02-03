use anyhow::{Context, Result};
use std::env;

mod backend;
mod blob;
mod crypto;
mod id;
mod index;
mod repo;

use backend::{DecryptBackend, FileType, LocalBackend, ReadBackend};
use blob::{Blob, BlobType};
use id::Id;
use index::ReadIndex;
use repo::*;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    let be = LocalBackend::new(&args[1]);
    let key = repo::find_key_in_backend(&be, &args[2], None)?;
    let be = DecryptBackend::new(be, key);

    let id = Id::from_hex(&args[3])?;
    let index = IndexFile::from_backend(&be, id)?;

    let blob = index
        .get_blob(Blob {
            tpe: BlobType::Tree,
            id: Id::from_hex("72e8cb97b980f840cd2fe0b0bfdaf8c7882fb93efef6e3130d199c004fd493ac")?,
        })
        .unwrap();

    let dec = be.read_partial(FileType::Pack, blob.pack, blob.bi.offset, blob.bi.length)?;
    println!("{}", String::from_utf8_lossy(&dec));

    // println!("{}", serde_json::to_string_pretty(&de).unwrap());

    Ok(())
}
