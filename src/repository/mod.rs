use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Context, Result};
use clap::Parser;
use log::*;
use merge::Merge;
use nom::{
    branch::alt,
    bytes::complete::{is_not, tag},
    character::complete::multispace1,
    error::ParseError,
    multi::separated_list0,
    sequence::delimited,
    IResult,
};
use rpassword::prompt_password;
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};

use crate::backend::{
    Cache, CachedBackend, ChooseBackend, DecryptBackend, DecryptReadBackend, DecryptWriteBackend,
    FileType, HotColdBackend, ReadBackend,
};
use crate::crypto::Key;
use crate::repofile::{find_key_in_backend, ConfigFile};

#[serde_as]
#[derive(Clone, Default, Debug, Parser, Deserialize, Merge)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct RepositoryOptions {
    /// Repository to use
    #[clap(short, long, global = true, alias = "repo", env = "RUSTIC_REPOSITORY")]
    repository: Option<String>,

    /// Repository to use as hot storage
    #[clap(long, global = true, alias = "repository_hot", env = "RUSTIC_REPO_HOT")]
    repo_hot: Option<String>,

    /// Password of the repository - WARNING: Using --password can reveal the password in the process list!
    #[clap(long, global = true, env = "RUSTIC_PASSWORD")]
    password: Option<String>,

    /// File to read the password from
    #[clap(
        short,
        long,
        global = true,
        env = "RUSTIC_PASSWORD_FILE",
        conflicts_with = "password"
    )]
    password_file: Option<PathBuf>,

    /// Command to read the password from
    #[clap(
        long,
        global = true,
        env = "RUSTIC_PASSWORD_COMMAND",
        conflicts_with_all = &["password", "password_file"],
    )]
    password_command: Option<String>,

    /// Don't use a cache.
    #[clap(long, global = true, env = "RUSTIC_NO_CACHE")]
    #[merge(strategy = merge::bool::overwrite_false)]
    no_cache: bool,

    /// Use this dir as cache dir instead of the standard cache dir
    #[clap(
        long,
        global = true,
        conflicts_with = "no_cache",
        env = "RUSTIC_CACHE_DIR"
    )]
    cache_dir: Option<PathBuf>,

    /// Warm up needed data pack files by only requesting them without processing
    #[clap(long, global = true)]
    #[merge(strategy = merge::bool::overwrite_false)]
    pub(crate) warm_up: bool,

    /// Warm up needed data pack files by running the command with %id replaced by pack id
    #[clap(long, global = true, conflicts_with = "warm_up")]
    pub(crate) warm_up_command: Option<String>,

    /// Duration (e.g. 10m) to wait after warm up
    #[clap(long, global = true, value_name = "DURATION")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub(crate) warm_up_wait: Option<humantime::Duration>,

    #[clap(skip)]
    #[merge(strategy = overwrite)]
    options: HashMap<String, String>,
}

fn overwrite<T>(left: &mut T, right: T) {
    *left = right;
}

// parse a command
pub(crate) fn parse_command<'a, E: ParseError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, Vec<&'a str>, E> {
    separated_list0(
        // a command is a list
        multispace1, // separated by one or more spaces
        alt((
            // and containing either
            delimited(tag("\""), is_not("\""), tag("\"")), // strings wrapped in "", or
            delimited(tag("'"), is_not("'"), tag("'")),    // strigns wrapped in '', or
            is_not(" \t\r\n"),                             // strings not containing any space
        )),
    )(input)
}

fn read_password_from_reader(file: &mut impl BufRead) -> std::io::Result<String> {
    let mut password = String::new();
    file.read_line(&mut password)?;

    // Remove the \n from the line if present
    if password.ends_with('\n') {
        password.pop();
    }

    // Remove the \r from the line if present
    if password.ends_with('\r') {
        password.pop();
    }

    Ok(password)
}

pub struct Repository {
    pub(crate) name: String,
    pub(crate) be: HotColdBackend<ChooseBackend>,
    pub(crate) be_hot: Option<ChooseBackend>,
    pub(crate) opts: RepositoryOptions,
}

impl Repository {
    pub fn new(opts: RepositoryOptions) -> Result<Self> {
        let be = match &opts.repository {
            Some(repo) => ChooseBackend::from_url(repo)?,
            None => bail!("No repository given. Please use the --repository option."),
        };

        if let Some(command) = &opts.warm_up_command {
            if !command.contains("%id") {
                bail!("warm-up command must contain %id!");
            }
            info!("using warm-up command {command}");
        }

        let be_hot = opts
            .repo_hot
            .as_ref()
            .map(|repo| ChooseBackend::from_url(repo))
            .transpose()?;

        let mut be = HotColdBackend::new(be, be_hot.clone());
        for (opt, value) in &opts.options {
            be.set_option(opt, value)?;
        }
        let mut name = be.location();
        if let Some(be_hot) = &be_hot {
            name.push('#');
            name.push_str(&be_hot.location());
        }

        Ok(Self {
            name,
            be,
            be_hot,
            opts,
        })
    }

    pub fn password(&self) -> Result<Option<String>> {
        match (
            &self.opts.password,
            &self.opts.password_file,
            &self.opts.password_command,
        ) {
            (Some(pwd), _, _) => Ok(Some(pwd.clone())),
            (_, Some(file), _) => {
                let mut file = BufReader::new(
                    File::open(file)
                        .with_context(|| format!("error opening password file {file:?}"))?,
                );
                Ok(Some(
                    read_password_from_reader(&mut file).context("error reading password file")?,
                ))
            }
            (_, _, Some(command)) => {
                let commands = parse_command::<()>(command)?.1;
                debug!("commands: {commands:?}");
                let output = Command::new(commands[0])
                    .args(&commands[1..])
                    .output()
                    .with_context(|| format!("failed to call password command {commands:?}"))?;

                let mut pwd = BufReader::new(&*output.stdout);
                Ok(Some(
                    read_password_from_reader(&mut pwd)
                        .context("error reading password from command")?,
                ))
            }
            (None, None, None) => Ok(None),
        }
    }

    pub fn open(self) -> Result<OpenRepository> {
        let config_ids = self
            .be
            .list(FileType::Config)
            .context("error listing the repo config file")?;

        match config_ids.len() {
            1 => {} // ok, continue
            0 => bail!(
                "No repository config file found. Is there a repo at {}?",
                self.name
            ),
            _ => bail!(
                "More than one repository config file at {}. Aborting.",
                self.name
            ),
        }

        if let Some(be_hot) = &self.be_hot {
            let mut keys = self
                .be
                .list_with_size(FileType::Key)
                .context("error listing the repo keys")?;
            keys.sort_unstable_by_key(|key| key.0);
            let mut hot_keys = be_hot
                .list_with_size(FileType::Key)
                .context("error listing the hot repo keys")?;
            hot_keys.sort_unstable_by_key(|key| key.0);
            if keys != hot_keys {
                bail!(
                    "keys from repo and repo-hot do not match for {}. Aborting.",
                    self.name
                );
            }
        }

        let key = get_key(&self.be, self.password()?)?;
        info!("repository {}: password is correct.", self.name);

        let dbe = DecryptBackend::new(&self.be, key.clone());
        let config: ConfigFile = dbe
            .get_file(&config_ids[0])
            .context("error accessing config file")?;
        match (config.is_hot == Some(true), self.be_hot.is_some()) {
                (true, false) => bail!("repository is a hot repository!\nPlease use as --repo-hot in combination with the normal repo. Aborting."),
                (false, true) => bail!("repo-hot is not a hot repository! Aborting."),
                _ => {}
            }
        let cache = (!self.opts.no_cache)
            .then(|| Cache::new(config.id, self.opts.cache_dir.clone()).ok())
            .flatten();
        match &cache {
            None => info!("using no cache"),
            Some(cache) => info!("using cache at {}", cache.location()),
        }
        let be_cached = CachedBackend::new(self.be.clone(), cache.clone());
        let mut dbe = DecryptBackend::new(&be_cached, key.clone());
        let zstd = config.zstd()?;
        dbe.set_zstd(zstd);

        Ok(OpenRepository {
            name: self.name,
            key,
            dbe,
            cache,
            be: self.be,
            be_hot: self.be_hot,
            config,
            opts: self.opts,
        })
    }
}

pub struct OpenRepository {
    pub(crate) name: String,
    pub(crate) be: HotColdBackend<ChooseBackend>,
    pub(crate) be_hot: Option<ChooseBackend>,
    pub(crate) key: Key,
    pub(crate) cache: Option<Cache>,
    pub(crate) dbe: DecryptBackend<CachedBackend<HotColdBackend<ChooseBackend>>, Key>,
    pub(crate) config: ConfigFile,
    pub(crate) opts: RepositoryOptions,
}

const MAX_PASSWORD_RETRIES: usize = 5;
pub fn get_key(be: &impl ReadBackend, password: Option<String>) -> Result<Key> {
    for _ in 0..MAX_PASSWORD_RETRIES {
        match password {
            // if password is given, directly return the result of find_key_in_backend and don't retry
            Some(pass) => return find_key_in_backend(be, &pass, None),
            None => {
                // TODO: Differentiate between wrong password and other error!
                if let Ok(key) =
                    find_key_in_backend(be, &prompt_password("enter repository password: ")?, None)
                {
                    return Ok(key);
                }
            }
        }
    }
    bail!("incorrect password!");
}
