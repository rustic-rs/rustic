use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::backend::{DecryptBackend, LocalBackend};
use crate::repo;

mod list;
mod cat;

#[derive(Parser)]
#[clap(about, version)]
struct Opts {
    /// repository
    #[clap(short, long)]
    repository: String,

    /// password
    #[clap(short, long)]
    password: String,

    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// list files
    List(list::Opts),
    
    /// cat files
    Cat(cat::Opts),
}

pub fn execute() -> Result<()> {
    let args = Opts::parse();

    let be = LocalBackend::new(&args.repository);
    let key = repo::find_key_in_backend(&be, &args.password, None)?;
    let be = DecryptBackend::new(&be, key);

    match args.command {
        Command::List(opts) => list::execute(&be, opts),
        Command::Cat(opts) => cat::execute(&be, opts),
    }
}
