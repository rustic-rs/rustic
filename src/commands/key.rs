use std::fs::File;
use std::io::BufReader;

use anyhow::Result;
use clap::{Parser, Subcommand};
use rpassword::{prompt_password, read_password_from_bufread};

use crate::backend::{FileType, WriteBackend};
use crate::crypto::{hash, Key};
use crate::repo::KeyFile;

#[derive(Parser)]
pub(super) struct Opts {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Add(AddOpts),
}

#[derive(Parser)]
pub(crate) struct AddOpts {
    /// set 'hostname' in public key information
    #[clap(long)]
    pub(crate) hostname: Option<String>,

    /// set 'username' in public key information
    #[clap(long)]
    pub(crate) username: Option<String>,

    /// add 'created' date in public key information
    #[clap(long)]
    pub(crate) with_created: bool,

    /// file from which to read the new password
    #[clap(long)]
    pub(crate) new_password_file: Option<String>,
}

pub(super) async fn execute(be: &impl WriteBackend, key: Key, opts: Opts) -> Result<()> {
    match opts.command {
        Command::Add(opt) => add_key(be, key, opt).await,
    }
}

async fn add_key(be: &impl WriteBackend, key: Key, opts: AddOpts) -> Result<()> {
    let pass = match opts.new_password_file {
        Some(file) => {
            let mut file = BufReader::new(File::open(file)?);
            read_password_from_bufread(&mut file)?
        }
        None => prompt_password("enter password for new key: ")?,
    };
    let keyfile = KeyFile::generate(key, &pass, opts.hostname, opts.username, opts.with_created)?;
    let data = serde_json::to_vec(&keyfile)?;
    let id = hash(&data);
    be.write_bytes(FileType::Key, &id, false, data).await?;

    println!("key {} successfully added.", id);
    Ok(())
}
