use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use ignore::WalkBuilder;
use path_absolutize::*;

use crate::archiver::Archiver;
use crate::backend::DecryptFullBackend;
use crate::index::IndexBackend;
use crate::repo::ConfigFile;

#[derive(Parser)]
pub(super) struct Opts {
    /// backup sources
    sources: Vec<String>,
}

pub(super) fn execute(opts: Opts, be: &impl DecryptFullBackend) -> Result<()> {
    let config = ConfigFile::from_backend_no_id(be)?;

    let poly = u64::from_str_radix(config.chunker_polynomial(), 16)?;
    let path = PathBuf::from(&opts.sources[0]);
    let path = path.absolutize()?;
    backup_file(path.into(), &poly, be)?;
    Ok(())
}

fn backup_file(backup_path: PathBuf, poly: &u64, be: &impl DecryptFullBackend) -> Result<()> {
    println! {"reading index..."}
    let index = IndexBackend::new(be);
    let mut archiver = Archiver::new(be.clone(), index, *poly)?;

    let mut wb = WalkBuilder::new(backup_path.clone());
    /*
     for path in paths[1..].into_iter() {
        wb.add(path);
    }
    */
    wb.follow_links(false).hidden(false);

    for entry in wb.build() {
        let entry = entry?;
        let name = entry.file_name().to_os_string();
        let file_type = entry.file_type().unwrap();
        println!("entry: {:?}", entry.path());

        archiver.add_entry(entry.path(), name, file_type)?;
    }
    archiver.finalize_snapshot(backup_path)?;

    Ok(())
}
