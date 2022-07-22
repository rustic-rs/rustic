use std::fs::File;
use std::io::BufReader;

use anyhow::{bail, Result};
use clap::Parser;
use rpassword::{prompt_password, read_password_from_bufread};

use super::config::ConfigOpts;
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

    #[clap(flatten)]
    config_opts: ConfigOpts,
}

pub(super) async fn execute(
    be: &impl WriteBackend,
    hot_be: &Option<impl WriteBackend>,
    opts: Opts,
    config_ids: Vec<Id>,
) -> Result<()> {
    if !config_ids.is_empty() {
        bail!("Config file already exists. Aborting.");
    }

    // Create config first to allow catching errors from here without writing anything
    let repo_id = Id::random();
    let chunker_poly = chunker::random_poly()?;
    let version = match opts.config_opts.set_version {
        None => 2,
        Some(_) => 1, // will be changed later
    };
    let mut config = ConfigFile::new(version, repo_id, chunker_poly);
    opts.config_opts.apply(&mut config)?;

    // generate key
    let key = Key::new();

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
    be.create().await?;
    be.write_bytes(FileType::Key, &id, data.clone()).await?;

    if let Some(hot_be) = hot_be {
        hot_be.create().await?;
        hot_be.write_bytes(FileType::Key, &id, data).await?;
    }
    println!("key {} successfully added.", id);

    // save config
    let dbe = DecryptBackend::new(be, key.clone());
    dbe.save_file(&config).await?;

    if let Some(hot_be) = hot_be {
        let dbe = DecryptBackend::new(hot_be, key);
        config.is_hot = Some(true);
        dbe.save_file(&config).await?;
    }
    println!("repository {} successfully created.", repo_id);

    Ok(())
}
