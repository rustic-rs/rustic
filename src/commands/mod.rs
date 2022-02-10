use std::fs;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::backend::{DecryptBackend, LocalBackend};
use crate::repo;

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
    #[clap(short, long)]
    password_file: String,

    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
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

pub fn execute() -> Result<()> {
    let args = Opts::parse();

    let be = LocalBackend::new(&args.repository);
    let passwd = fs::read_to_string(&args.password_file)?.replace("\n", "");
    let key = repo::find_key_in_backend(&be, &passwd, None)?;
    let dbe = DecryptBackend::new(&be, key);

    match args.command {
        Command::Cat(opts) => cat::execute(&be, &dbe, opts),
        Command::Check(opts) => check::execute(&dbe, opts),
        Command::Diff(opts) => diff::execute(&dbe, opts),
        Command::List(opts) => list::execute(&dbe, opts),
        Command::Ls(opts) => ls::execute(&dbe, opts),
        Command::Snapshots(opts) => snapshots::execute(&dbe, opts),
        Command::Restore(opts) => restore::execute(&dbe, opts),
    }
}
