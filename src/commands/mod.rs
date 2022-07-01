use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};

use crate::backend::{
    Cache, CachedBackend, ChooseBackend, DecryptBackend, DecryptReadBackend, FileType, ReadBackend,
};
use crate::repo::ConfigFile;

mod backup;
mod cat;
mod check;
mod diff;
mod forget;
mod helpers;
mod init;
mod key;
mod list;
mod ls;
mod prune;
mod repoinfo;
mod restore;
mod snapshots;
mod tag;

use helpers::*;
use vlog::*;

#[derive(Parser)]
#[clap(about, version)]
struct Opts {
    /// repository
    #[clap(short, long)]
    repository: String,

    /// file to read the password from
    #[clap(short, long, parse(from_os_str))]
    password_file: Option<PathBuf>,

    #[clap(long, short = 'v', parse(from_occurrences))]
    verbose: i8,

    #[clap(long, short = 'q', parse(from_occurrences), conflicts_with = "verbose")]
    quiet: i8,

    /// Don't create a cache dir for the repo. Note that existing cache dirs will be
    /// always used.
    #[clap(long)]
    no_cache: bool,

    /// Use this thir as cache dir. If not given, rustic searches for restic cache dirs
    /// and rustic cache dirs
    #[clap(long, parse(from_os_str), conflicts_with = "no-cache")]
    cache_dir: Option<PathBuf>,

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

    /// remove snapshots from the repository
    Forget(forget::Opts),

    /// initialize a new repository
    Init(init::Opts),

    /// manage keys
    Key(key::Opts),

    /// list repository files
    List(list::Opts),

    /// ls snapshots
    Ls(ls::Opts),

    /// show snapshots
    Snapshots(snapshots::Opts),

    /// remove unused data
    Prune(prune::Opts),

    /// restore snapshot
    Restore(restore::Opts),

    /// show general information about repository
    Repoinfo(repoinfo::Opts),

    /// change tags of snapshots
    Tag(tag::Opts),
}

pub async fn execute() -> Result<()> {
    let command: Vec<_> = std::env::args_os().into_iter().collect();
    let args = Opts::parse_from(&command);
    let command: String = command
        .into_iter()
        .map(|s| s.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(" ");

    let verbosity = (1 + args.verbose - args.quiet).clamp(0, 3);
    set_verbosity_level(verbosity as usize);

    let be = ChooseBackend::from_url(&args.repository);

    let config_ids = be.list(FileType::Config).await?;

    let (cmd, key, dbe, cache, be, config) = match (args.command, config_ids.len()) {
        (Command::Init(opts), 0) => return init::execute(&be, opts).await,
        (Command::Init(_), _) => bail!("Config file already exists. Aborting."),
        (cmd, 1) => {
            let key = get_key(&be, args.password_file).await?;
            let dbe = DecryptBackend::new(&be, key.clone());
            let config: ConfigFile = dbe.get_file(&config_ids[0]).await?;
            let cache = Cache::new(config.id, args.cache_dir, !args.no_cache)?;
            match &cache {
                None => v1!("using no cache"),
                Some(cache) => v1!("using cache at {}", cache.location()),
            }
            let be_cached = CachedBackend::new(be.clone(), cache.clone());
            let dbe = DecryptBackend::new(&be_cached, key.clone());
            (cmd, key, dbe, cache, be, config)
        }
        (_, 0) => bail!("No config file found. Is there a repo?"),
        _ => bail!("More than one config file. Aborting."),
    };

    match cmd {
        Command::Backup(opts) => backup::execute(&dbe, opts, config, command).await?,
        Command::Cat(opts) => cat::execute(&dbe, opts).await?,
        Command::Check(opts) => check::execute(&dbe, &cache, &be, opts).await?,
        Command::Diff(opts) => diff::execute(&dbe, opts).await?,
        Command::Forget(opts) => forget::execute(&dbe, opts, config).await?,
        Command::Init(_) => {} // already handled above
        Command::Key(opts) => key::execute(&dbe, key, opts).await?,
        Command::List(opts) => list::execute(&dbe, opts).await?,
        Command::Ls(opts) => ls::execute(&dbe, opts).await?,
        Command::Snapshots(opts) => snapshots::execute(&dbe, opts).await?,
        Command::Prune(opts) => prune::execute(&dbe, opts, config, vec![]).await?,
        Command::Restore(opts) => restore::execute(&dbe, opts).await?,
        Command::Repoinfo(opts) => repoinfo::execute(&dbe, opts).await?,
        Command::Tag(opts) => tag::execute(&dbe, opts).await?,
    };

    Ok(())
}
