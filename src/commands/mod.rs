use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use rpassword::{prompt_password_stderr, read_password_with_reader};

use crate::backend::{DecryptBackend, LocalBackend};
use crate::repo;

mod backup;
mod cat;
mod check;
mod diff;
mod list;
mod ls;
mod restore;
mod snapshots;

#[derive(Parser)]
#[clap(about, version)]
struct Opts {
    /// repository
    #[clap(short, long)]
    repository: String,

    /// file to read the password from
    #[clap(short, long, parse(from_os_str))]
    password_file: Option<PathBuf>,

    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// backup to the repository
    Backup(backup::Opts),

    /// cat repository files and blobs
    Cat(cat::Opts),

    /// check repository
    Check(check::Opts),

    /// compare two snapshots
    Diff(diff::Opts),

    /// list repository files
    List(list::Opts),

    /// ls snapshots
    Ls(ls::Opts),

    /// show snapshots
    Snapshots(snapshots::Opts),

    /// restore snapshot
    Restore(restore::Opts),
}

const MAX_PASSWORD_RETRIES: usize = 5;

pub fn execute() -> Result<()> {
    let args = Opts::parse();

    let be = LocalBackend::new(&args.repository);

    let key = match args.password_file {
        None => (0..MAX_PASSWORD_RETRIES)
            .map(|_| {
                let pass = prompt_password_stderr("enter repository password: ")?;
                repo::find_key_in_backend(&be, &pass, None)
            })
            .find(Result::is_ok)
            .unwrap_or_else(|| bail!("tried too often...aborting!"))?,
        Some(file) => {
            let pass = fs::read_to_string(file)?.replace("\n", "");
            repo::find_key_in_backend(&be, &pass, None)?
        }
    };
    eprintln!("password is correct");

    let dbe = DecryptBackend::new(&be, key.clone());

    match args.command {
        Command::Backup(opts) => backup::execute(opts, &dbe, &key),
        Command::Cat(opts) => cat::execute(&be, &dbe, opts),
        Command::Check(opts) => check::execute(&dbe, opts),
        Command::Diff(opts) => diff::execute(&dbe, opts),
        Command::List(opts) => list::execute(&dbe, opts),
        Command::Ls(opts) => ls::execute(&dbe, opts),
        Command::Snapshots(opts) => snapshots::execute(&dbe, opts),
        Command::Restore(opts) => restore::execute(&dbe, opts),
    }
}
