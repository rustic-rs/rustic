//! `backup` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{
    commands::{get_repository, open_repository},
    {status_err, Application, RUSTIC_APP},
};
use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::{bail, Result};
use log::{debug, info, warn};

use std::path::PathBuf;
use std::str::FromStr;

use chrono::Local;

use merge::Merge;
use path_dedot::ParseDot;
use serde::Deserialize;

use rustic_core::{
    bytes_size_to_string, Archiver, DryRunBackend, IndexBackend, LocalSource,
    LocalSourceFilterOptions, LocalSourceSaveOptions, PathList, SnapshotFile, SnapshotGroup,
    SnapshotGroupCriterion, SnapshotOptions, StdinSource,
};

/// `backup` subcommand
#[derive(Clone, Command, Default, Debug, clap::Parser, Deserialize, Merge)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
// Note: using cli_sources, sources and source within this strict is a hack to support serde(deny_unknown_fields)
// for deserializing the backup options from TOML
// Unfortunately we cannot work with nested flattened structures, see
// https://github.com/serde-rs/serde/issues/1547
// A drawback is that a wrongly set "source(s) = ..." won't get correct error handling and need to be manually checked, see below.
#[allow(clippy::struct_excessive_bools)]
pub struct BackupCmd {
    /// Backup source (can be specified multiple times), use - for stdin. If no source is given, uses all
    /// sources defined in the config file
    #[clap(value_name = "SOURCE")]
    #[merge(skip)]
    #[serde(skip)]
    cli_sources: Vec<String>,

    /// Group snapshots by any combination of host,label,paths,tags to find a suitable parent (default: host,label,paths)
    #[clap(
        long,
        short = 'g',
        value_name = "CRITERION",
        help_heading = "Options for parent processing"
    )]
    group_by: Option<SnapshotGroupCriterion>,

    /// Snapshot to use as parent
    #[clap(
        long,
        value_name = "SNAPSHOT",
        conflicts_with = "force",
        help_heading = "Options for parent processing"
    )]
    parent: Option<String>,

    /// Use no parent, read all files
    #[clap(
        long,
        short,
        conflicts_with = "parent",
        help_heading = "Options for parent processing"
    )]
    #[merge(strategy = merge::bool::overwrite_false)]
    force: bool,

    /// Ignore ctime changes when checking for modified files
    #[clap(
        long,
        conflicts_with = "force",
        help_heading = "Options for parent processing"
    )]
    #[merge(strategy = merge::bool::overwrite_false)]
    ignore_ctime: bool,

    /// Ignore inode number changes when checking for modified files
    #[clap(
        long,
        conflicts_with = "force",
        help_heading = "Options for parent processing"
    )]
    #[merge(strategy = merge::bool::overwrite_false)]
    ignore_inode: bool,

    /// Set filename to be used when backing up from stdin
    #[clap(long, value_name = "FILENAME", default_value = "stdin")]
    #[merge(skip)]
    stdin_filename: String,

    /// Manually set backup path in snapshot
    #[clap(long, value_name = "PATH")]
    as_path: Option<PathBuf>,

    #[clap(flatten)]
    #[serde(flatten)]
    ignore_save_opts: LocalSourceSaveOptions,

    #[clap(flatten)]
    #[serde(flatten)]
    ignore_filter_opts: LocalSourceFilterOptions,

    #[clap(flatten, next_help_heading = "Snapshot options")]
    #[serde(flatten)]
    snap_opts: SnapshotOptions,

    /// Output generated snapshot in json format
    #[clap(long)]
    #[merge(strategy = merge::bool::overwrite_false)]
    json: bool,

    #[clap(skip)]
    #[merge(strategy = merge_sources)]
    sources: Vec<BackupCmd>,

    /// Backup source, used within config file
    #[clap(skip)]
    #[merge(skip)]
    source: String,
}

// Merge backup sources: If a source is already defined on left, use that. Else add it.
pub(crate) fn merge_sources(left: &mut Vec<BackupCmd>, mut right: Vec<BackupCmd>) {
    left.append(&mut right);
    left.sort_by(|opt1, opt2| opt1.source.cmp(&opt2.source));
    left.dedup_by(|opt1, opt2| opt1.source == opt2.source);
}

impl Runnable for BackupCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl BackupCmd {
    fn inner_run(&self) -> Result<()> {
        let time = Local::now();

        let command: String = std::env::args_os()
            .map(|s| s.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(" ");

        let config = RUSTIC_APP.config();
        let progress_options = &config.global.progress_options;

        let repo = open_repository(get_repository(&config));

        // manually check for a "source" field, check is not done by serde, see above.
        if !config.backup.source.is_empty() {
            bail!("key \"source\" is not valid in the [backup] section!");
        }

        let config_opts = &config.backup.sources;

        // manually check for a "sources" field, check is not done by serde, see above.
        if config_opts.iter().any(|opt| !opt.sources.is_empty()) {
            bail!("key \"sources\" is not valid in a [[backup.sources]] section!");
        }

        let config_sources: Vec<_> = config_opts
            .iter()
            .filter_map(|opt| match PathList::from_string(&opt.source, true) {
                Ok(paths) => Some(paths),
                Err(err) => {
                    warn!(
                        "error sanitizing source=\"{}\" in config file: {err}",
                        opt.source
                    );
                    None
                }
            })
            .collect();

        let sources = match (self.cli_sources.is_empty(), config_opts.is_empty()) {
            (false, _) => {
                let item = PathList::from_strings(&self.cli_sources, true)?;
                vec![item]
            }
            (true, false) => {
                info!("using all backup sources from config file.");
                config_sources.clone()
            }
            (true, true) => {
                bail!("no backup source given.");
            }
        };

        let index =
            IndexBackend::only_full_trees(&repo.dbe, progress_options.progress_counter(""))?;

        for source in sources {
            let mut opts = self.clone();
            let index = index.clone();
            let backup_stdin = source == PathList::from_string("-", false)?;
            let backup_path = if backup_stdin {
                vec![PathBuf::from(&opts.stdin_filename)]
            } else {
                source.paths()
            };

            // merge Options from config file, if given
            if let Some(idx) = config_sources.iter().position(|s| s == &source) {
                info!("merging source={source} section from config file");
                opts.merge(config_opts[idx].clone());
            }
            if let Some(path) = &opts.as_path {
                // as_path only works in combination with a single target
                if source.len() > 1 {
                    bail!("as-path only works with a single target!");
                }
                // merge Options from config file using as_path, if given
                if let Some(path) = path.as_os_str().to_str() {
                    if let Some(idx) = config_opts.iter().position(|opt| opt.source == path) {
                        info!("merging source=\"{path}\" section from config file");
                        opts.merge(config_opts[idx].clone());
                    }
                }
            }

            // merge "backup" section from config file, if given
            opts.merge(config.backup.clone());

            let be = DryRunBackend::new(repo.dbe.clone(), config.global.dry_run);
            info!("starting to backup {source}...");
            let as_path = opts.as_path.map(|p| {
                match p.parse_dot() {
                    Ok(it) => it,
                    Err(err) => {
                        status_err!("{}", err);
                        RUSTIC_APP.shutdown(Shutdown::Crash);
                    }
                }
                .to_path_buf()
            });

            let mut snap = SnapshotFile::new_from_options(&opts.snap_opts, time, command.clone())?;
            match &as_path {
                Some(p) => snap.paths.set_paths(&[p.clone()])?,
                None => snap.paths.set_paths(&backup_path)?,
            };

            // get suitable snapshot group from snapshot and opts.group_by. This is used to filter snapshots for the parent detection
            let group = SnapshotGroup::from_sn(
                &snap,
                &opts.group_by.unwrap_or_else(|| {
                    SnapshotGroupCriterion::from_str("host,label,paths").unwrap()
                }),
            );

            let parent = match (backup_stdin, opts.force, opts.parent.clone()) {
                (true, _, _) | (false, true, _) => None,
                (false, false, None) => SnapshotFile::latest(
                    &be,
                    |snap| snap.has_group(&group),
                    &progress_options.progress_counter(""),
                )
                .ok(),
                (false, false, Some(parent)) => SnapshotFile::from_id(&be, &parent).ok(),
            };

            let parent_tree = match &parent {
                Some(parent) => {
                    info!("using parent {}", parent.id);
                    snap.parent = Some(parent.id);
                    Some(parent.tree)
                }
                None => {
                    info!("using no parent");
                    None
                }
            };

            let archiver = Archiver::new(
                be,
                index,
                &repo.config,
                parent_tree,
                opts.ignore_ctime,
                opts.ignore_inode,
                snap,
            )?;
            let p = progress_options.progress_bytes("determining size...");

            let snap = if backup_stdin {
                let path = &backup_path[0];
                let src = StdinSource::new(path.clone())?;
                archiver.archive(src, path, as_path.as_ref(), &p)?
            } else {
                let src = LocalSource::new(
                    opts.ignore_save_opts,
                    opts.ignore_filter_opts.clone(),
                    &backup_path,
                )?;
                archiver.archive(src, &backup_path[0], as_path.as_ref(), &p)?
            };

            if opts.json {
                let mut stdout = std::io::stdout();
                serde_json::to_writer_pretty(&mut stdout, &snap)?;
            } else {
                let summary = snap.summary.unwrap();
                println!(
                    "Files:       {} new, {} changed, {} unchanged",
                    summary.files_new, summary.files_changed, summary.files_unmodified
                );
                println!(
                    "Dirs:        {} new, {} changed, {} unchanged",
                    summary.dirs_new, summary.dirs_changed, summary.dirs_unmodified
                );
                debug!("Data Blobs:  {} new", summary.data_blobs);
                debug!("Tree Blobs:  {} new", summary.tree_blobs);
                println!(
                    "Added to the repo: {} (raw: {})",
                    bytes_size_to_string(summary.data_added_packed),
                    bytes_size_to_string(summary.data_added)
                );

                println!(
                    "processed {} files, {}",
                    summary.total_files_processed,
                    bytes_size_to_string(summary.total_bytes_processed)
                );
                println!("snapshot {} successfully saved.", snap.id);
            }

            info!("backup of {source} done.");
        }

        Ok(())
    }
}
