use std::fs;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::backend::{DecryptBackend, LocalBackend};
use crate::repo;

mod cat;
mod check;
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
    #[clap(short, long)]
    password_file: String,

    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// list files
    List(list::Opts),

    /// cat files
    Cat(cat::Opts),

    /// show snapshots
    Snapshots(snapshots::Opts),

    /// ls snapshots
    Ls(ls::Opts),

    /// check repository
    Check(check::Opts),

    /// restore snapshot
    Restore(restore::Opts),
}

pub fn execute() -> Result<()> {
    let args = Opts::parse();

    let be = LocalBackend::new(&args.repository);
    let passwd = fs::read_to_string(&args.password_file)?.replace("\n", "");
    let key = repo::find_key_in_backend(&be, &passwd, None)?;
    let dbe = DecryptBackend::new(&be, key);

    match args.command {
        Command::List(opts) => list::execute(&dbe, opts),
        Command::Cat(opts) => cat::execute(&be, &dbe, opts),
        Command::Snapshots(opts) => snapshots::execute(&dbe, opts),
        Command::Ls(opts) => ls::execute(&dbe, opts),
        Command::Check(opts) => check::execute(&dbe, opts),
        Command::Restore(opts) => restore::execute(&dbe, opts),
    }
}
