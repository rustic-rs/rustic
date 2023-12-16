//! `backup` subcommand

use std::path::PathBuf;

use crate::{
    commands::open_repository,
    helpers::bytes_size_to_string,
    {status_err, Application, RUSTIC_APP},
};
use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::{bail, Context, Result};
use log::{debug, info, warn};

use merge::Merge;
use serde::Deserialize;

use rustic_core::{
    BackupOptions, ConfigOptions, KeyOptions, LocalSourceFilterOptions, LocalSourceSaveOptions,
    ParentOptions, PathList, Repository, SnapshotOptions,
};

use super::init::init;

/// `backup` subcommand
#[derive(Clone, Command, Default, Debug, clap::Parser, Deserialize, Merge)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
// Note: using cli_sources, sources and source within this struct is a hack to support serde(deny_unknown_fields)
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

    /// Set filename to be used when backing up from stdin
    #[clap(long, value_name = "FILENAME", default_value = "stdin")]
    #[merge(skip)]
    stdin_filename: String,

    /// Manually set backup path in snapshot
    #[clap(long, value_name = "PATH")]
    as_path: Option<PathBuf>,

    /// Ignore save options
    #[clap(flatten)]
    #[serde(flatten)]
    ignore_save_opts: LocalSourceSaveOptions,

    /// Don't scan the backup source for its size - this disables ETA estimation for backup.
    #[clap(long)]
    #[merge(strategy = merge::bool::overwrite_false)]
    pub no_scan: bool,

    /// Output generated snapshot in json format
    #[clap(long)]
    #[merge(strategy = merge::bool::overwrite_false)]
    json: bool,

    /// Don't show any output
    #[clap(long, conflicts_with = "json")]
    #[merge(strategy = merge::bool::overwrite_false)]
    quiet: bool,

    /// Initialize repository, if it doesn't exist yet
    #[clap(long)]
    #[merge(strategy = merge::bool::overwrite_false)]
    init: bool,

    /// Parent processing options
    #[clap(flatten, next_help_heading = "Options for parent processing")]
    #[serde(flatten)]
    parent_opts: ParentOptions,

    /// Exclude options
    #[clap(flatten, next_help_heading = "Exclude options")]
    #[serde(flatten)]
    ignore_filter_opts: LocalSourceFilterOptions,

    /// Snapshot options
    #[clap(flatten, next_help_heading = "Snapshot options")]
    #[serde(flatten)]
    snap_opts: SnapshotOptions,

    /// Key options (when using --init)
    #[clap(flatten, next_help_heading = "Key options (when using --init)")]
    #[serde(skip)]
    #[merge(skip)]
    key_opts: KeyOptions,

    /// Config options (when using --init)
    #[clap(flatten, next_help_heading = "Config options (when using --init)")]
    #[serde(skip)]
    #[merge(skip)]
    config_opts: ConfigOptions,

    /// Backup sources
    #[clap(skip)]
    #[merge(strategy = merge_sources)]
    sources: Vec<BackupCmd>,

    /// Backup source, used within config file
    #[clap(skip)]
    #[merge(skip)]
    source: String,
}

/// Merge backup sources
///
/// If a source is already defined on left, use that. Else add it.
///
/// # Arguments
///
/// * `left` - Vector of backup sources
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
        let config = RUSTIC_APP.config();

        let po = config.global.progress_options;
        let repo = Repository::new_with_progress(&config.repository, po)?;
        // Initialize repository if --init is set and it is not yet initialized
        let repo = if self.init && repo.config_id()?.is_none() {
            if config.global.dry_run {
                bail!(
                    "cannot initialize repository {} in dry-run mode!",
                    repo.name
                );
            }
            init(repo, &self.key_opts, &self.config_opts)?
        } else {
            open_repository(&config)?
        }
        .to_indexed_ids()?;

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
            .map(|opt| -> Result<_> {
                Ok(PathList::from_string(&opt.source)?
                    .sanitize()
                    .with_context(|| {
                        format!("error sanitizing source=\"{}\" in config file", opt.source)
                    })?
                    .merge())
            })
            .filter_map(|p| match p {
                Ok(paths) => Some(paths),
                Err(err) => {
                    warn!("{err}");
                    None
                }
            })
            .collect();

        let sources = match (self.cli_sources.is_empty(), config_opts.is_empty()) {
            (false, _) => {
                let item = PathList::from_strings(&self.cli_sources).sanitize()?;
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

        for source in sources {
            let mut opts = self.clone();

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

            let backup_opts = BackupOptions::default()
                .stdin_filename(opts.stdin_filename)
                .as_path(opts.as_path)
                .parent_opts(opts.parent_opts)
                .ignore_save_opts(opts.ignore_save_opts)
                .ignore_filter_opts(opts.ignore_filter_opts)
                .no_scan(opts.no_scan)
                .dry_run(config.global.dry_run);
            let snap = repo.backup(&backup_opts, source.clone(), opts.snap_opts.to_snapshot()?)?;

            if opts.json {
                let mut stdout = std::io::stdout();
                serde_json::to_writer_pretty(&mut stdout, &snap)?;
            } else if !opts.quiet {
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
