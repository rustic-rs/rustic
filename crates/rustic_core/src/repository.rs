use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use bytes::Bytes;
use derive_setters::Setters;
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

use serde_with::{serde_as, DisplayFromStr};

use crate::{
    backend::{
        cache::Cache,
        cache::CachedBackend,
        choose::ChooseBackend,
        decrypt::{DecryptBackend, DecryptFullBackend, DecryptReadBackend, DecryptWriteBackend},
        hotcold::HotColdBackend,
        FileType, ReadBackend,
    },
    commands::{
        self,
        backup::BackupOpts,
        check::CheckOpts,
        config::ConfigOpts,
        copy::CopySnapshot,
        forget::{ForgetGroups, KeepOptions},
        key::KeyOpts,
        repoinfo::{IndexInfos, RepoFileInfos},
        restore::{RestoreInfos, RestoreOpts},
    },
    crypto::aespoly1305::Key,
    error::{KeyFileErrorKind, RepositoryErrorKind, RusticErrorKind},
    repofile::RepoFile,
    repofile::{configfile::ConfigFile, keyfile::find_key_in_backend},
    BlobType, Id, IndexBackend, IndexedBackend, LocalDestination, NoProgressBars, Node,
    NodeStreamer, PathList, ProgressBars, PruneOpts, PrunePlan, RusticResult, SnapshotFile,
    SnapshotGroup, SnapshotGroupCriterion, Tree, TreeStreamerOptions,
};

mod warm_up;
use warm_up::{warm_up, warm_up_wait};

#[serde_as]
#[cfg_attr(feature = "clap", derive(clap::Parser))]
#[cfg_attr(feature = "merge", derive(merge::Merge))]
#[derive(Clone, Default, Debug, serde::Deserialize, Setters)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
#[setters(into, strip_option)]
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

pub fn read_password_from_reader(file: &mut impl BufRead) -> RusticResult<String> {
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

#[derive(Debug, Clone)]
pub struct Repository<P, S> {
    pub name: String,
    pub be: HotColdBackend<ChooseBackend>,
    pub be_hot: Option<ChooseBackend>,
    opts: RepositoryOptions,
    pub(crate) pb: P,
    status: S,
}

impl Repository<NoProgressBars, ()> {
    pub fn new(opts: &RepositoryOptions) -> RusticResult<Self> {
        Self::new_with_progress(opts, NoProgressBars {})
    }
}

impl<P> Repository<P, ()> {
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
            status: (),
        })
    }
}
impl<P, S> Repository<P, S> {
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

    pub fn config_id(&self) -> RusticResult<Option<Id>> {
        let config_ids = self
            .be
            .list(FileType::Config)
            .map_err(|_| RepositoryErrorKind::ListingRepositoryConfigFileFailed)?;

        match config_ids.len() {
            1 => Ok(Some(config_ids[0])),
            0 => Ok(None),
            _ => Err(RepositoryErrorKind::MoreThanOneRepositoryConfig(self.name.clone()).into()),
        }
    }

    pub fn open(self) -> RusticResult<Repository<P, OpenStatus>> {
        let password = self
            .password()?
            .ok_or(RepositoryErrorKind::NoPasswordGiven)?;
        self.open_with_password(&password)
    }

    pub fn open_with_password(self, password: &str) -> RusticResult<Repository<P, OpenStatus>> {
        let config_id = self
            .config_id()?
            .ok_or(RepositoryErrorKind::NoRepositoryConfigFound(
                self.name.clone(),
            ))?;

        if let Some(be_hot) = &self.be_hot {
            let mut keys = self.be.list_with_size(FileType::Key)?;
            keys.sort_unstable_by_key(|key| key.0);
            let mut hot_keys = be_hot.list_with_size(FileType::Key)?;
            hot_keys.sort_unstable_by_key(|key| key.0);
            if keys != hot_keys {
                return Err(RepositoryErrorKind::KeysDontMatchForRepositories(self.name).into());
            }
        }

        let key = find_key_in_backend(&self.be, &password, None).map_err(|err| {
            match err.into_inner() {
                RusticErrorKind::KeyFile(KeyFileErrorKind::NoSuitableKeyFound) => {
                    RepositoryErrorKind::IncorrectPassword.into()
                }
                err => err,
            }
        })?;
        info!("repository {}: password is correct.", self.name);
        let dbe = DecryptBackend::new(&self.be, key);
        let config: ConfigFile = dbe.get_file(&config_id)?;
        self.open_raw(key, config)
    }

    pub fn init(
        self,
        key_opts: &KeyOpts,
        config_opts: &ConfigOpts,
    ) -> RusticResult<Repository<P, OpenStatus>> {
        let password = self
            .password()?
            .ok_or(RepositoryErrorKind::NoPasswordGiven)?;
        self.init_with_password(&password, key_opts, config_opts)
    }

    pub fn init_with_password(
        self,
        pass: &str,
        key_opts: &KeyOpts,
        config_opts: &ConfigOpts,
    ) -> RusticResult<Repository<P, OpenStatus>> {
        if self.config_id()?.is_some() {
            return Err(RepositoryErrorKind::ConfigFileExists.into());
        }
        let (key, config) = commands::init::init(&self, pass, key_opts, config_opts)?;
        self.open_raw(key, config)
    }

    pub fn init_with_config(
        self,
        pass: &str,
        key_opts: &KeyOpts,
        config: ConfigFile,
    ) -> RusticResult<Repository<P, OpenStatus>> {
        let key = commands::init::init_with_config(&self, pass, key_opts, &config)?;
        info!("repository {} successfully created.", config.id);
        self.open_raw(key, config)
    }

    fn open_raw(self, key: Key, config: ConfigFile) -> RusticResult<Repository<P, OpenStatus>> {
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

        let open = OpenStatus {
            key,
            dbe,
            cache,
            config,
        };

        Ok(Repository {
            name: self.name,
            be: self.be,
            be_hot: self.be_hot,
            opts: self.opts,
            pb: self.pb,
            status: open,
        })
    }

    pub fn list(&self, tpe: FileType) -> RusticResult<impl Iterator<Item = Id>> {
        Ok(self.be.list(tpe)?.into_iter())
    }
}

impl<P: ProgressBars, S> Repository<P, S> {
    pub fn infos_files(&self) -> RusticResult<RepoFileInfos> {
        commands::repoinfo::collect_file_infos(self)
    }
    pub fn warm_up(&self, packs: impl ExactSizeIterator<Item = Id>) -> RusticResult<()> {
        warm_up(self, packs)
    }

    pub fn warm_up_wait(&self, packs: impl ExactSizeIterator<Item = Id>) -> RusticResult<()> {
        warm_up_wait(self, packs)
    }
}

pub trait Open {
    type DBE: DecryptFullBackend;
    fn key(&self) -> &Key;
    fn cache(&self) -> Option<&Cache>;
    fn dbe(&self) -> &Self::DBE;
    fn config(&self) -> &ConfigFile;
}

impl<P, S: Open> Open for Repository<P, S> {
    type DBE = S::DBE;
    fn key(&self) -> &Key {
        self.status.key()
    }
    fn cache(&self) -> Option<&Cache> {
        self.status.cache()
    }
    fn dbe(&self) -> &Self::DBE {
        self.status.dbe()
    }
    fn config(&self) -> &ConfigFile {
        self.status.config()
    }
}

#[derive(Debug)]
pub struct OpenStatus {
    key: Key,
    cache: Option<Cache>,
    dbe: DecryptBackend<CachedBackend<HotColdBackend<ChooseBackend>>, Key>,
    config: ConfigFile,
}

impl Open for OpenStatus {
    type DBE = DecryptBackend<CachedBackend<HotColdBackend<ChooseBackend>>, Key>;

    fn key(&self) -> &Key {
        &self.key
    }
    fn cache(&self) -> Option<&Cache> {
        self.cache.as_ref()
    }
    fn dbe(&self) -> &Self::DBE {
        &self.dbe
    }
    fn config(&self) -> &ConfigFile {
        &self.config
    }
}

impl<P, S: Open> Repository<P, S> {
    pub fn cat_file(&self, tpe: FileType, id: &str) -> RusticResult<Bytes> {
        commands::cat::cat_file(self, tpe, id)
    }

    pub fn add_key(&self, pass: &str, opts: &KeyOpts) -> RusticResult<Id> {
        opts.add_key(self, pass)
    }

    pub fn apply_config(&self, opts: &ConfigOpts) -> RusticResult<bool> {
        commands::config::apply_config(self, opts)
    }
}

impl<P: ProgressBars, S: Open> Repository<P, S> {
    pub fn get_snapshot_group(
        &self,
        ids: &[String],
        group_by: SnapshotGroupCriterion,
        filter: impl FnMut(&SnapshotFile) -> bool,
    ) -> RusticResult<Vec<(SnapshotGroup, Vec<SnapshotFile>)>> {
        commands::snapshots::get_snapshot_group(self, ids, group_by, filter)
    }

    pub fn get_snapshots(&self, ids: &[String]) -> RusticResult<Vec<SnapshotFile>> {
        let p = self.pb.progress_counter("getting snapshots...");
        SnapshotFile::from_ids(self.dbe(), ids, &p)
    }

    pub fn get_all_snapshots(&self) -> RusticResult<Vec<SnapshotFile>> {
        self.get_matching_snapshots(|_| true)
    }

    pub fn get_matching_snapshots(
        &self,
        filter: impl FnMut(&SnapshotFile) -> bool,
    ) -> RusticResult<Vec<SnapshotFile>> {
        let p = self.pb.progress_counter("getting snapshots...");
        SnapshotFile::all_from_backend(self.dbe(), filter, &p)
    }

    pub fn get_forget_snapshots(
        &self,
        keep: &KeepOptions,
        group_by: SnapshotGroupCriterion,
        filter: impl FnMut(&SnapshotFile) -> bool,
    ) -> RusticResult<ForgetGroups> {
        commands::forget::get_forget_snapshots(self, keep, group_by, filter)
    }

    pub fn relevant_copy_snapshots(
        &self,
        filter: impl FnMut(&SnapshotFile) -> bool,
        snaps: &[SnapshotFile],
    ) -> RusticResult<Vec<CopySnapshot>> {
        commands::copy::relevant_snapshots(snaps, self, filter)
    }

    pub fn delete_snapshots(&self, ids: &[Id]) -> RusticResult<()> {
        let p = self.pb.progress_counter("removing snapshots...");
        self.dbe()
            .delete_list(FileType::Snapshot, true, ids.iter(), p)?;
        Ok(())
    }

    pub fn save_snapshots(&self, mut snaps: Vec<SnapshotFile>) -> RusticResult<()> {
        for snap in &mut snaps {
            snap.id = Id::default();
        }
        let p = self.pb.progress_counter("saving snapshots...");
        self.dbe().save_list(snaps.iter(), p)?;
        Ok(())
    }

    pub fn check(&self, opts: CheckOpts) -> RusticResult<()> {
        opts.run(self)
    }

    pub fn prune_plan(&self, opts: &PruneOpts) -> RusticResult<PrunePlan> {
        opts.get_plan(self)
    }

    pub fn to_indexed(self) -> RusticResult<Repository<P, IndexedStatus<FullIndex, S>>> {
        let index = IndexBackend::new(self.dbe(), &self.pb.progress_counter(""))?;
        let status = IndexedStatus {
            open: self.status,
            index,
            marker: std::marker::PhantomData,
        };
        Ok(Repository {
            name: self.name,
            be: self.be,
            be_hot: self.be_hot,
            opts: self.opts,
            pb: self.pb,
            status,
        })
    }

    pub fn to_indexed_ids(self) -> RusticResult<Repository<P, IndexedStatus<IdIndex, S>>> {
        let index = IndexBackend::only_full_trees(self.dbe(), &self.pb.progress_counter(""))?;
        let status = IndexedStatus {
            open: self.status,
            index,
            marker: std::marker::PhantomData,
        };
        Ok(Repository {
            name: self.name,
            be: self.be,
            be_hot: self.be_hot,
            opts: self.opts,
            pb: self.pb,
            status,
        })
    }

    pub fn infos_index(&self) -> RusticResult<IndexInfos> {
        commands::repoinfo::collect_index_infos(self)
    }

    pub fn stream_files<F: RepoFile>(
        &self,
    ) -> RusticResult<impl Iterator<Item = RusticResult<(Id, F)>>> {
        Ok(self
            .dbe()
            .stream_all::<F>(&self.pb.progress_hidden())?
            .into_iter())
    }
}

pub trait IndexedTree: Open {
    type I: IndexedBackend;
    fn index(&self) -> &Self::I;
}

pub trait IndexedIds: IndexedTree {}
pub trait IndexedFull: IndexedIds {}

impl<P, S: IndexedTree> IndexedTree for Repository<P, S> {
    type I = S::I;
    fn index(&self) -> &Self::I {
        self.status.index()
    }
}

#[derive(Debug)]
pub struct IndexedStatus<T, S: Open> {
    open: S,
    index: IndexBackend<S::DBE>,
    marker: std::marker::PhantomData<T>,
}

#[derive(Debug, Clone, Copy)]
pub struct IdIndex {}
#[derive(Debug, Clone, Copy)]
pub struct FullIndex {}

impl<T, S: Open> IndexedTree for IndexedStatus<T, S> {
    type I = IndexBackend<S::DBE>;

    fn index(&self) -> &Self::I {
        &self.index
    }
}

impl<S: Open> IndexedIds for IndexedStatus<IdIndex, S> {}
impl<S: Open> IndexedIds for IndexedStatus<FullIndex, S> {}
impl<S: Open> IndexedFull for IndexedStatus<FullIndex, S> {}

impl<T, S: Open> Open for IndexedStatus<T, S> {
    type DBE = S::DBE;

    fn key(&self) -> &Key {
        self.open.key()
    }
    fn cache(&self) -> Option<&Cache> {
        self.open.cache()
    }
    fn dbe(&self) -> &Self::DBE {
        self.open.dbe()
    }
    fn config(&self) -> &ConfigFile {
        self.open.config()
    }
}

impl<P: ProgressBars, S: IndexedTree> Repository<P, S> {
    pub fn node_from_snapshot_path(
        &self,
        snap_path: &str,
        filter: impl FnMut(&SnapshotFile) -> bool + Send + Sync,
    ) -> RusticResult<Node> {
        let (id, path) = snap_path.split_once(':').unwrap_or((snap_path, ""));

        let p = &self.pb.progress_counter("getting snapshot...");
        let snap = SnapshotFile::from_str(self.dbe(), id, filter, p)?;

        Tree::node_from_path(self.index(), snap.tree, Path::new(path))
    }

    pub fn cat_tree(
        &self,
        snap: &str,
        sn_filter: impl FnMut(&SnapshotFile) -> bool + Send + Sync,
    ) -> RusticResult<Bytes> {
        commands::cat::cat_tree(self, snap, sn_filter)
    }

    pub fn ls(
        &self,
        node: &Node,
        streamer_opts: &TreeStreamerOptions,
        recursive: bool,
    ) -> RusticResult<impl Iterator<Item = RusticResult<(PathBuf, Node)>> + Clone> {
        NodeStreamer::new_with_glob(self.index().clone(), node, streamer_opts, recursive)
    }

    pub fn restore(
        &self,
        restore_infos: RestoreInfos,
        opts: &RestoreOpts,
        node_streamer: impl Iterator<Item = RusticResult<(PathBuf, Node)>>,
        dest: &LocalDestination,
    ) -> RusticResult<()> {
        opts.restore(restore_infos, self, node_streamer, dest)
    }
}

impl<P: ProgressBars, S: IndexedIds> Repository<P, S> {
    pub fn backup(
        &self,
        opts: &BackupOpts,
        source: PathList,
        snap: SnapshotFile,
        dry_run: bool,
    ) -> RusticResult<SnapshotFile> {
        commands::backup::backup(self, opts, source, snap, dry_run)
    }
}

impl<P: ProgressBars, S: IndexedFull> Repository<P, S> {
    pub fn cat_blob(&self, tpe: BlobType, id: &str) -> RusticResult<Bytes> {
        commands::cat::cat_blob(self, tpe, id)
    }

    pub fn dump(&self, node: &Node, w: &mut impl Write) -> RusticResult<()> {
        commands::dump::dump(self, node, w)
    }

    /// Prepare the restore.
    /// If `dry_run` is set to false, it will also:
    /// - remove existing files from the destination, if `opts.delete` is set to true
    /// - create all dirs for the restore
    pub fn prepare_restore(
        &self,
        opts: &RestoreOpts,
        node_streamer: impl Iterator<Item = RusticResult<(PathBuf, Node)>>,
        dest: &LocalDestination,
        dry_run: bool,
    ) -> RusticResult<RestoreInfos> {
        opts.collect_and_prepare(self, node_streamer, dest, dry_run)
    }

    /// Copy the given `snapshots` to `repo_dest`.
    /// Note: This command copies snapshots even if they already exist. For already existing snapshots, a
    /// copy will be created in the destination repository.
    /// To omit already existing snapshots, use `relevante_copy_snapshots` and filter out the non-relevant ones.
    pub fn copy<'a, Q: ProgressBars, R: IndexedIds>(
        &self,
        repo_dest: &Repository<Q, R>,
        snapshots: impl IntoIterator<Item = &'a SnapshotFile>,
    ) -> RusticResult<()> {
        commands::copy::copy(self, repo_dest, snapshots)
    }
}
