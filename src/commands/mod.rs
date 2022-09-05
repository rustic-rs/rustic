use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use merge::Merge;
use serde::Deserialize;

use crate::backend::{
    Cache, CachedBackend, ChooseBackend, DecryptBackend, DecryptReadBackend, FileType,
    HotColdBackend, ReadBackend,
};
use crate::repo::ConfigFile;

mod backup;
mod cat;
mod check;
mod config;
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
mod rustic_config;
mod self_update;
mod snapshots;
mod tag;

use helpers::*;
use rustic_config::RusticConfig;
use vlog::*;

#[derive(Parser)]
#[clap(about, version)]
struct Opts {
    #[clap(flatten, help_heading = "GLOBAL OPTIONS")]
    global: GlobalOpts,

    /// Config profile to use. This parses the file <PROFILE>.toml in the config directory.
    #[clap(
        short = 'P',
        long,
        value_name = "PROFILE",
        global = true,
        default_value = "rustic"
    )]
    config_profile: String,

    #[clap(subcommand)]
    command: Command,
}

#[derive(Default, Parser, Deserialize, Merge)]
#[serde(default, rename_all = "kebab-case")]
struct GlobalOpts {
    /// Repository to use
    #[clap(short, long, global = true, env = "RUSTIC_REPOSITORY")]
    repository: Option<String>,

    /// Repository to use as hot storage
    #[clap(long, global = true, env = "RUSTIC_REPO_HOT")]
    repo_hot: Option<String>,

    /// Password of the repository - WARNING: Using --password can reveal the password in the process list!
    #[clap(long, global = true, env = "RUSTIC_PASSWORD")]
    password: Option<String>,

    /// File to read the password from
    #[clap(
        short,
        long,
        global = true,
        parse(from_os_str),
        env = "RUSTIC_PASSWORD_FILE",
        conflicts_with = "password"
    )]
    password_file: Option<PathBuf>,

    /// Command to read the password from
    #[clap(
        long,
        global = true,
        env = "RUSTIC_PASSWORD_COMMAND",
        conflicts_with_all = &["password", "password-file"],
    )]
    password_command: Option<String>,

    /// Increase verbosity (can be used multiple times)
    #[clap(long, short = 'v', global = true, parse(from_occurrences))]
    #[merge(strategy = merge::num::overwrite_zero)]
    verbose: i8,

    /// Don't be verbose at all
    #[clap(
        long,
        short = 'q',
        global = true,
        parse(from_occurrences),
        conflicts_with = "verbose"
    )]
    #[merge(strategy = merge::num::overwrite_zero)]
    quiet: i8,

    /// Don't use a cache.
    #[clap(long, global = true, env = "RUSTIC_NO_CACHE")]
    #[merge(strategy = merge::bool::overwrite_false)]
    no_cache: bool,

    /// Use this dir as cache dir instead of the standard cache dir
    #[clap(
        long,
        global = true,
        parse(from_os_str),
        conflicts_with = "no-cache",
        env = "RUSTIC_CACHE_DIR"
    )]
    cache_dir: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Backup to the repository
    Backup(backup::Opts),

    /// Show raw data of repository files and blobs
    Cat(cat::Opts),

    /// Change the repository configuration
    Config(config::Opts),

    /// Check the repository
    Check(check::Opts),

    /// Compare two snapshots/paths
    Diff(diff::Opts),

    /// Remove snapshots from the repository
    Forget(forget::Opts),

    /// Initialize a new repository
    Init(init::Opts),

    /// Manage keys
    Key(key::Opts),

    /// List repository files
    List(list::Opts),

    /// List file contents of a snapshot
    Ls(ls::Opts),

    /// Show a detailed overview of the snapshots within the repository
    Snapshots(snapshots::Opts),

    /// Update to the latest rustic release
    SelfUpdate(self_update::Opts),

    /// Remove unused data or repack repository pack files
    Prune(prune::Opts),

    /// Restore a snapshot/path
    Restore(restore::Opts),

    /// Show general information about the repository
    Repoinfo(repoinfo::Opts),

    /// Change tags of snapshots
    Tag(tag::Opts),
}

pub async fn execute() -> Result<()> {
    let command: Vec<_> = std::env::args_os().into_iter().collect();
    let args = Opts::parse_from(&command);

    let config_file = RusticConfig::new(&args.config_profile)?;

    let mut opts = args.global;
    config_file.merge_into("global", &mut opts)?;

    if let Command::SelfUpdate(opts) = args.command {
        self_update::execute(opts).await?;
        return Ok(());
    }

    let command: String = command
        .into_iter()
        .map(|s| s.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(" ");

    let verbosity = (1 + opts.verbose - opts.quiet).clamp(0, 3);
    set_verbosity_level(verbosity as usize);

    let be = match &opts.repository {
        Some(repo) => ChooseBackend::from_url(repo)?,
        None => bail!("No repository given. Please use the --repository option."),
    };

    let be_hot = opts
        .repo_hot
        .map(|repo| ChooseBackend::from_url(&repo))
        .transpose()?;

    let config_ids = be.list(FileType::Config).await?;

    let (cmd, key, dbe, cache, be, be_hot, config) = match (args.command, config_ids.len()) {
        (Command::Init(opts), _) => return init::execute(&be, &be_hot, opts, config_ids).await,
        (cmd, 1) => {
            let be = HotColdBackend::new(be, be_hot.clone());
            if let Some(be_hot) = &be_hot {
                let mut keys = be.list_with_size(FileType::Key).await?;
                keys.sort_unstable_by_key(|key| key.0);
                let mut hot_keys = be_hot.list_with_size(FileType::Key).await?;
                hot_keys.sort_unstable_by_key(|key| key.0);
                if keys != hot_keys {
                    bail!("keys from repo and repo-hot do not match. Aborting.");
                }
            }

            let key = get_key(
                &be,
                opts.password.as_deref(),
                opts.password_file.as_deref(),
                opts.password_command.as_deref(),
            )
            .await?;
            ve1!("password is correct.");

            let dbe = DecryptBackend::new(&be, key.clone());
            let config: ConfigFile = dbe.get_file(&config_ids[0]).await?;
            match (config.is_hot == Some(true), be_hot.is_some()) {
                (true, false) => bail!("repository is a hot repository!\nPlease use as --repo-hot in combination with the normal repo. Aborting."),
                (false, true) => bail!("repo-hot is not a hot repository! Aborting."),
                _ => {}
            }
            let cache = (!opts.no_cache)
                .then(|| Cache::new(config.id, opts.cache_dir).ok())
                .flatten();
            match &cache {
                None => v1!("using no cache"),
                Some(cache) => v1!("using cache at {}", cache.location()),
            }
            let be_cached = CachedBackend::new(be.clone(), cache.clone());
            let dbe = DecryptBackend::new(&be_cached, key.clone());
            (cmd, key, dbe, cache, be, be_hot, config)
        }
        (_, 0) => bail!("No config file found. Is there a repo?"),
        _ => bail!("More than one config file. Aborting."),
    };

    match cmd {
        Command::Backup(opts) => backup::execute(&dbe, opts, config, config_file, command).await?,
        Command::Config(opts) => config::execute(&dbe, &be_hot, opts, config).await?,
        Command::Cat(opts) => cat::execute(&dbe, opts).await?,
        Command::Check(opts) => check::execute(&dbe, &cache, &be_hot, &be, opts).await?,
        Command::Diff(opts) => diff::execute(&dbe, opts).await?,
        Command::Forget(opts) => forget::execute(&dbe, cache, opts, config, config_file).await?,
        Command::Init(_) => {} // already handled above
        Command::Key(opts) => key::execute(&dbe, key, opts).await?,
        Command::List(opts) => list::execute(&dbe, opts).await?,
        Command::Ls(opts) => ls::execute(&dbe, opts).await?,
        Command::SelfUpdate(_) => {} // already handled above
        Command::Snapshots(opts) => snapshots::execute(&dbe, opts, config_file).await?,
        Command::Prune(opts) => prune::execute(&dbe, cache, opts, config, vec![]).await?,
        Command::Restore(opts) => restore::execute(&dbe, opts).await?,
        Command::Repoinfo(opts) => repoinfo::execute(&dbe, &be_hot, opts).await?,
        Command::Tag(opts) => tag::execute(&dbe, opts, config_file).await?,
    };

    Ok(())
}
