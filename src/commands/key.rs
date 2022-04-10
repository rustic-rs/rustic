use std::fs::File;
use std::io::BufReader;

use anyhow::Result;
use clap::{Parser, Subcommand};
use rpassword::{prompt_password_stderr, read_password_with_reader};

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
struct AddOpts {
    /// set 'hostname' in public key information
    #[clap(long)]
    hostname: Option<String>,

    /// set 'username' in public key information
    #[clap(long)]
    username: Option<String>,

    /// add 'created' date in public key information
    #[clap(long)]
    with_created: bool,

    /// file from which to read the new password
    #[clap(long)]
    new_password_file: Option<String>,
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
            read_password_with_reader(Some(&mut file))?
        }
        None => prompt_password_stderr("enter password for new key: ")?,
    };
    let keyfile = KeyFile::generate(key, &pass, opts.hostname, opts.username, opts.with_created)?;
    let data = serde_json::to_vec(&keyfile)?;
    let id = hash(&data);
    be.write_bytes(FileType::Key, &id, data).await?;

    println!("key {} successfully added.", id);
    Ok(())
}
