use std::fs;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::backend::{DecryptBackend, LocalBackend};
use crate::repo;

mod cat;
mod list;
mod ls;
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

    /// cat files
    Snapshots(snapshots::Opts),

    /// ls snapshots
    Ls(ls::Opts),
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
    }
}
