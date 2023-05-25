use std::fs::File;
use std::io::BufReader;

use anyhow::Result;
use clap::{Parser, Subcommand};
use rpassword::{prompt_password, read_password_from_bufread};

use crate::backend::{FileType, WriteBackend};
use crate::crypto::{hash, Key};
use crate::repofile::KeyFile;
use crate::repository::OpenRepository;

#[derive(Parser)]
pub(super) struct Opts {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Add a new key to the repository
    Add(AddOpts),
}

#[derive(Parser)]
pub(crate) struct AddOpts {
    /// File from which to read the new password
    #[clap(long)]
    pub(crate) new_password_file: Option<String>,

    #[clap(flatten)]
    pub key_opts: KeyOpts,
}

#[derive(Clone, Parser)]
pub(crate) struct KeyOpts {
    /// Set 'hostname' in public key information
    #[clap(long)]
    pub(crate) hostname: Option<String>,

    /// Set 'username' in public key information
    #[clap(long)]
    pub(crate) username: Option<String>,

    /// Add 'created' date in public key information
    #[clap(long)]
    pub(crate) with_created: bool,
}

pub(super) fn execute(repo: OpenRepository, opts: Opts) -> Result<()> {
    match opts.command {
        Command::Add(opt) => add_key(&repo.dbe, repo.key, opt),
    }
}

fn add_key(be: &impl WriteBackend, key: Key, opts: AddOpts) -> Result<()> {
    let pass = match opts.new_password_file {
        Some(file) => {
            let mut file = BufReader::new(File::open(file)?);
            read_password_from_bufread(&mut file)?
        }
        None => prompt_password("enter password for new key: ")?,
    };
    let ko = opts.key_opts;
    let keyfile = KeyFile::generate(key, &pass, ko.hostname, ko.username, ko.with_created)?;
    let data = serde_json::to_vec(&keyfile)?;
    let id = hash(&data);
    be.write_bytes(FileType::Key, &id, false, data.into())?;

    println!("key {id} successfully added.");
    Ok(())
}
