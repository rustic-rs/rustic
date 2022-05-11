use std::fs::File;
use std::io::BufReader;

use anyhow::Result;
use clap::Parser;
use rpassword::{prompt_password, read_password_from_bufread};

use super::key::AddOpts;
use crate::backend::{DecryptBackend, DecryptWriteBackend, FileType, WriteBackend};
use crate::chunker;
use crate::crypto::{hash, Key};
use crate::id::Id;
use crate::repo::{ConfigFile, KeyFile};

#[derive(Parser)]
pub(super) struct Opts {
    #[clap(flatten)]
    key_opts: AddOpts,
}

pub(super) async fn execute(be: &impl WriteBackend, opts: Opts) -> Result<()> {
    let key = Key::new();

    be.create().await?;

    let key_opts = opts.key_opts;
    let pass = match key_opts.new_password_file {
        Some(file) => {
            let mut file = BufReader::new(File::open(file)?);
            read_password_from_bufread(&mut file)?
        }
        None => prompt_password("enter password for new key: ")?,
    };
    let keyfile = KeyFile::generate(
        key.clone(),
        &pass,
        key_opts.hostname,
        key_opts.username,
        key_opts.with_created,
    )?;
    let data = serde_json::to_vec(&keyfile)?;
    let id = hash(&data);
    be.write_bytes(FileType::Key, &id, data).await?;
    println!("key {} successfully added.", id);

    let dbe = DecryptBackend::new(be, key);
    let repo_id = Id::random();
    let chunker_poly = chunker::random_poly()?;
    let config = ConfigFile::new(1, repo_id, chunker_poly);
    dbe.save_file(&config).await?;
    println!("repository {} successfully created.", repo_id);

    Ok(())
}
