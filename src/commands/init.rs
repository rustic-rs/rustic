use anyhow::{bail, Result};
use bytes::Bytes;
use clap::Parser;
use rpassword::prompt_password;

use super::config::ConfigOpts;
use super::key::KeyOpts;
use crate::backend::{DecryptBackend, DecryptWriteBackend, FileType, WriteBackend};
use crate::chunker;
use crate::crypto::{hash, Key};
use crate::id::Id;
use crate::repofile::{ConfigFile, KeyFile};

#[derive(Parser)]
pub(super) struct Opts {
    #[clap(flatten, next_help_heading = "Key options")]
    key_opts: KeyOpts,

    #[clap(flatten, next_help_heading = "Config options")]
    config_opts: ConfigOpts,
}

pub(super) fn execute(
    be: &impl WriteBackend,
    hot_be: &Option<impl WriteBackend>,
    opts: Opts,
    password: Option<String>,
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

    let pass = match password {
        Some(pass) => pass,
        None => prompt_password("enter password for new key: ")?,
    };

    let key_opts = opts.key_opts;
    let keyfile = KeyFile::generate(
        key.clone(),
        &pass,
        key_opts.hostname,
        key_opts.username,
        key_opts.with_created,
    )?;
    let data: Bytes = serde_json::to_vec(&keyfile)?.into();
    let id = hash(&data);
    be.create()?;
    be.write_bytes(FileType::Key, &id, false, data)?;
    println!("key {id} successfully added.");

    // save config
    let dbe = DecryptBackend::new(be, key.clone());
    dbe.save_file(&config)?;

    if let Some(hot_be) = hot_be {
        let dbe = DecryptBackend::new(hot_be, key);
        config.is_hot = Some(true);
        dbe.save_file(&config)?;
    }
    println!("repository {repo_id} successfully created.");

    Ok(())
}
