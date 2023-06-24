use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Command, Stdio},
};

use bytes::Bytes;
use log::{debug, error, info};

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
    commands::{
        self,
        check::CheckOpts,
        repoinfo::{IndexInfos, RepoFileInfos},
    },
    crypto::aespoly1305::Key,
    error::RepositoryErrorKind,
    repofile::{configfile::ConfigFile, keyfile::find_key_in_backend},
    BlobType, IndexBackend, NoProgressBars, ProgressBars, PruneOpts, PrunePlan, RusticResult,
    SnapshotFile,
};

pub(super) mod constants {
    pub(super) const MAX_PASSWORD_RETRIES: usize = 5;
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
    pub repository: Option<String>,

    /// Repository to use as hot storage
    #[cfg_attr(
        feature = "clap",
        clap(long, global = true, alias = "repository_hot", env = "RUSTIC_REPO_HOT")
    )]
    pub repo_hot: Option<String>,

    /// Password of the repository - WARNING: Using --password can reveal the password in the process list!
    #[cfg_attr(feature = "clap", clap(long, global = true, env = "RUSTIC_PASSWORD"))]
    // TODO: use `secrecy` library
    pub password: Option<String>,

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
    pub password_file: Option<PathBuf>,

    /// Command to read the password from. Password is read from stdout
    #[cfg_attr(feature = "clap", clap(
        long,
        global = true,
        env = "RUSTIC_PASSWORD_COMMAND",
        conflicts_with_all = &["password", "password_file"],
    ))]
    pub password_command: Option<String>,

    /// Don't use a cache.
    #[cfg_attr(feature = "clap", clap(long, global = true, env = "RUSTIC_NO_CACHE"))]
    #[cfg_attr(feature = "merge", merge(strategy = merge::bool::overwrite_false))]
    pub no_cache: bool,

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
    pub cache_dir: Option<PathBuf>,

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
    pub options: HashMap<String, String>,
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
pub struct Repository<P> {
    name: String,
    pub be: HotColdBackend<ChooseBackend>,
    pub be_hot: Option<ChooseBackend>,
    opts: RepositoryOptions,
    pub(crate) pb: P,
}

impl Repository<NoProgressBars> {
    pub fn new(opts: &RepositoryOptions) -> RusticResult<Self> {
        Self::new_with_progress(opts, NoProgressBars {})
    }
}

impl<P> Repository<P> {
    pub fn new_with_progress(opts: &RepositoryOptions, pb: P) -> RusticResult<Self> {
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
            pb,
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
                let command = Command::new(commands[0])
                    .args(&commands[1..])
                    .stdout(Stdio::piped())
                    .spawn()?;
                let Ok(output) = command.wait_with_output() else {
                    return Err(RepositoryErrorKind::PasswordCommandParsingFailed.into());
                };
                if !output.status.success() {
                    #[allow(clippy::option_if_let_else)]
                    let s = match output.status.code() {
                        Some(c) => format!("exited with status code {c}"),
                        None => "was terminated".into(),
                    };
                    error!("password-command {s}");
                    return Err(RepositoryErrorKind::ReadingPasswordFromCommandFailed.into());
                }

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

    pub fn open(self) -> RusticResult<OpenRepository<P>> {
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
            pb: self.pb,
        })
    }
}

impl<P: ProgressBars> Repository<P> {
    pub fn infos_files(&self) -> RusticResult<RepoFileInfos> {
        commands::repoinfo::collect_file_infos(self)
    }
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

#[derive(Debug)]
pub struct OpenRepository<P> {
    pub name: String,
    pub be: HotColdBackend<ChooseBackend>,
    pub be_hot: Option<ChooseBackend>,
    pub key: Key,
    pub cache: Option<Cache>,
    pub dbe: DecryptBackend<CachedBackend<HotColdBackend<ChooseBackend>>, Key>,
    pub config: ConfigFile,
    pub opts: RepositoryOptions,
    pub(crate) pb: P,
}

impl<P: ProgressBars> OpenRepository<P> {
    pub fn cat_file(&self, tpe: FileType, id: &str) -> RusticResult<Bytes> {
        commands::cat::cat_file(self, tpe, id)
    }

    pub fn check(&self, opts: CheckOpts) -> RusticResult<()> {
        opts.run(self)
    }

    pub fn prune_plan(&self, opts: &PruneOpts) -> RusticResult<PrunePlan> {
        opts.get_plan(self)
    }

    pub fn to_indexed(self) -> RusticResult<IndexedRepository<P>> {
        let index = IndexBackend::new(&self.dbe, &self.pb.progress_counter(""))?;
        Ok(IndexedRepository { repo: self, index })
    }

    pub fn infos_index(&self) -> RusticResult<IndexInfos> {
        commands::repoinfo::collect_index_infos(self)
    }
}

#[derive(Debug)]
pub struct IndexedRepository<P> {
    pub(crate) repo: OpenRepository<P>,
    pub(crate) index:
        IndexBackend<DecryptBackend<CachedBackend<HotColdBackend<ChooseBackend>>, Key>>,
}

impl<P: ProgressBars> IndexedRepository<P> {
    pub fn cat_blob(&self, tpe: BlobType, id: &str) -> RusticResult<Bytes> {
        commands::cat::cat_blob(self, tpe, id)
    }
    pub fn cat_tree(
        &self,
        snap: &str,
        sn_filter: impl FnMut(&SnapshotFile) -> bool + Send + Sync,
    ) -> RusticResult<Bytes> {
        commands::cat::cat_tree(self, snap, sn_filter)
    }
}
