use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::Command,
};

use derive_more::Add;
use log::{debug, info};

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

use serde_with::{serde_as, DisplayFromStr};

use crate::{
    backend::{
        cache::Cache, cache::CachedBackend, choose::ChooseBackend, decrypt::DecryptBackend,
        decrypt::DecryptReadBackend, decrypt::DecryptWriteBackend, hotcold::HotColdBackend,
        FileType, ReadBackend,
    },
    crypto::aespoly1305::Key,
    error::RepositoryErrorKind,
    index::IndexEntry,
    repofile::{configfile::ConfigFile, indexfile::IndexPack, keyfile::find_key_in_backend},
    RusticResult,
};

pub(super) mod constants {
    pub(super) const MAX_PASSWORD_RETRIES: usize = 5;
}

#[derive(Default, Clone, Copy, Add, Debug)]
pub struct RepoInfo {
    pub count: u64,
    pub size: u64,
    pub data_size: u64,
    pub pack_count: u64,
    pub total_pack_size: u64,
    pub min_pack_size: u64,
    pub max_pack_size: u64,
}

impl RepoInfo {
    pub fn add(&mut self, ie: IndexEntry) {
        self.count += 1;
        self.size += u64::from(ie.length);
        self.data_size += u64::from(ie.data_length());
    }

    pub fn add_pack(&mut self, ip: &IndexPack) {
        self.pack_count += 1;
        let size = u64::from(ip.pack_size());
        self.total_pack_size += size;
        self.min_pack_size = self.min_pack_size.min(size);
        self.max_pack_size = self.max_pack_size.max(size);
    }
}

#[serde_as]
#[cfg_attr(feature = "clap", derive(clap::Parser))]
#[cfg_attr(feature = "merge", derive(merge::Merge))]
#[derive(Clone, Default, Debug, serde::Deserialize)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct RepositoryOptions {
    /// Repository to use
    #[cfg_attr(
        feature = "clap",
        clap(short, long, global = true, alias = "repo", env = "RUSTIC_REPOSITORY")
    )]
    repository: Option<String>,

    /// Repository to use as hot storage
    #[cfg_attr(
        feature = "clap",
        clap(long, global = true, alias = "repository_hot", env = "RUSTIC_REPO_HOT")
    )]
    repo_hot: Option<String>,

    /// Password of the repository - WARNING: Using --password can reveal the password in the process list!
    #[cfg_attr(feature = "clap", clap(long, global = true, env = "RUSTIC_PASSWORD"))]
    // TODO: use `secrecy` library
    password: Option<String>,

    /// File to read the password from
    #[cfg_attr(
        feature = "clap",
        clap(
            short,
            long,
            global = true,
            env = "RUSTIC_PASSWORD_FILE",
            conflicts_with = "password"
        )
    )]
    password_file: Option<PathBuf>,

    /// Command to read the password from
    #[cfg_attr(feature = "clap", clap(
        long,
        global = true,
        env = "RUSTIC_PASSWORD_COMMAND",
        conflicts_with_all = &["password", "password_file"],
    ))]
    password_command: Option<String>,

    /// Don't use a cache.
    #[cfg_attr(feature = "clap", clap(long, global = true, env = "RUSTIC_NO_CACHE"))]
    #[cfg_attr(feature = "merge", merge(strategy = merge::bool::overwrite_false))]
    no_cache: bool,

    /// Use this dir as cache dir instead of the standard cache dir
    #[cfg_attr(
        feature = "clap",
        clap(
            long,
            global = true,
            conflicts_with = "no_cache",
            env = "RUSTIC_CACHE_DIR"
        )
    )]
    cache_dir: Option<PathBuf>,

    /// Warm up needed data pack files by only requesting them without processing
    #[cfg_attr(feature = "clap", clap(long, global = true))]
    #[cfg_attr(feature = "merge", merge(strategy = merge::bool::overwrite_false))]
    pub warm_up: bool,

    /// Warm up needed data pack files by running the command with %id replaced by pack id
    #[cfg_attr(
        feature = "clap",
        clap(long, global = true, conflicts_with = "warm_up")
    )]
    pub warm_up_command: Option<String>,

    /// Duration (e.g. 10m) to wait after warm up
    #[cfg_attr(feature = "clap", clap(long, global = true, value_name = "DURATION"))]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub warm_up_wait: Option<humantime::Duration>,

    #[cfg_attr(feature = "clap", clap(skip))]
    #[cfg_attr(feature = "merge", merge(strategy = overwrite))]
    options: HashMap<String, String>,
}

// TODO: Unused function
#[allow(dead_code)]
pub(crate) fn overwrite<T>(left: &mut T, right: T) {
    *left = right;
}

// parse a command
pub fn parse_command<'a, E: ParseError<&'a str>>(
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

pub(crate) fn read_password_from_reader(file: &mut impl BufRead) -> RusticResult<String> {
    let mut password = String::new();
    _ = file
        .read_line(&mut password)
        .map_err(RepositoryErrorKind::ReadingPasswordFromReaderFailed)?;

    // Remove the \n from the line if present
    if password.ends_with('\n') {
        _ = password.pop();
    }

    // Remove the \r from the line if present
    if password.ends_with('\r') {
        _ = password.pop();
    }

    Ok(password)
}

#[derive(Debug)]
pub struct Repository {
    name: String,
    pub be: HotColdBackend<ChooseBackend>,
    pub be_hot: Option<ChooseBackend>,
    opts: RepositoryOptions,
}

impl Repository {
    pub fn new(opts: &RepositoryOptions) -> RusticResult<Self> {
        let be = match &opts.repository {
            Some(repo) => ChooseBackend::from_url(repo)?,
            None => return Err(RepositoryErrorKind::NoRepositoryGiven.into()),
        };

        if let Some(command) = &opts.warm_up_command {
            if !command.contains("%id") {
                return Err(RepositoryErrorKind::NoIDSpecified.into());
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
            opts: opts.clone(),
        })
    }

    pub fn password(&self) -> RusticResult<Option<String>> {
        match (
            &self.opts.password,
            &self.opts.password_file,
            &self.opts.password_command,
        ) {
            (Some(pwd), _, _) => Ok(Some(pwd.clone())),
            (_, Some(file), _) => {
                let mut file = BufReader::new(
                    File::open(file).map_err(RepositoryErrorKind::OpeningPasswordFileFailed)?,
                );
                Ok(Some(read_password_from_reader(&mut file)?))
            }
            (_, _, Some(command)) => {
                let commands = parse_command::<()>(command)
                    .map_err(RepositoryErrorKind::FromNomError)?
                    .1;
                debug!("commands: {commands:?}");
                let Ok(output) = Command::new(commands[0]).args(&commands[1..]).output() else {
                        return Err(
                            RepositoryErrorKind::PasswordCommandParsingFailed.into());
                    };

                let mut pwd = BufReader::new(&*output.stdout);
                Ok(Some(match read_password_from_reader(&mut pwd) {
                    Ok(val) => val,
                    Err(_) => {
                        return Err(RepositoryErrorKind::ReadingPasswordFromCommandFailed.into())
                    }
                }))
            }
            (None, None, None) => Ok(None),
        }
    }

    pub fn open(self) -> RusticResult<OpenRepository> {
        let config_ids = match self.be.list(FileType::Config) {
            Ok(val) => val,
            Err(_e) => return Err(RepositoryErrorKind::ListingRepositoryConfigFileFailed.into()),
        };

        match config_ids.len() {
            1 => {} // ok, continue
            0 => return Err(RepositoryErrorKind::NoRepositoryConfigFound(self.name).into()),
            _ => return Err(RepositoryErrorKind::MoreThanOneRepositoryConfig(self.name).into()),
        }

        if let Some(be_hot) = &self.be_hot {
            let mut keys = self.be.list_with_size(FileType::Key)?;
            keys.sort_unstable_by_key(|key| key.0);
            let mut hot_keys = be_hot.list_with_size(FileType::Key)?;
            hot_keys.sort_unstable_by_key(|key| key.0);
            if keys != hot_keys {
                return Err(RepositoryErrorKind::KeysDontMatchForRepositories(self.name).into());
            }
        }

        let key = get_key(&self.be, self.password()?)?;
        info!("repository {}: password is correct.", self.name);

        let dbe = DecryptBackend::new(&self.be, key);
        let config: ConfigFile = dbe.get_file(&config_ids[0])?;
        match (config.is_hot == Some(true), self.be_hot.is_some()) {
            (true, false) => return Err(RepositoryErrorKind::HotRepositoryFlagMissing.into()),
            (false, true) => return Err(RepositoryErrorKind::IsNotHotRepository.into()),
            _ => {}
        }
        let cache = (!self.opts.no_cache)
            .then(|| Cache::new(config.id, self.opts.cache_dir.clone()).ok())
            .flatten();
        cache.as_ref().map_or_else(
            || info!("using no cache"),
            |cache| info!("using cache at {}", cache.location()),
        );
        let be_cached = CachedBackend::new(self.be.clone(), cache.clone());
        let mut dbe = DecryptBackend::new(&be_cached, key);
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

#[derive(Debug)]
pub struct OpenRepository {
    pub name: String,
    pub be: HotColdBackend<ChooseBackend>,
    pub be_hot: Option<ChooseBackend>,
    pub key: Key,
    pub cache: Option<Cache>,
    pub dbe: DecryptBackend<CachedBackend<HotColdBackend<ChooseBackend>>, Key>,
    pub config: ConfigFile,
    pub opts: RepositoryOptions,
}

pub(crate) fn get_key(be: &impl ReadBackend, password: Option<String>) -> RusticResult<Key> {
    for _ in 0..constants::MAX_PASSWORD_RETRIES {
        match password {
            // if password is given, directly return the result of find_key_in_backend and don't retry
            Some(pass) => {
                return find_key_in_backend(be, &pass, None).map_err(std::convert::Into::into)
            }
            None => {
                // TODO: Differentiate between wrong password and other error!
                if let Ok(key) = find_key_in_backend(
                    be,
                    &prompt_password("enter repository password: ")
                        .map_err(RepositoryErrorKind::ReadingPasswordFromPromptFailed)?,
                    None,
                ) {
                    return Ok(key);
                }
            }
        }
    }
    Err(RepositoryErrorKind::IncorrectPassword.into())
}
