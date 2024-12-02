//! `backup` subcommand

use std::path::PathBuf;

use crate::{
    commands::{init::init, snapshots::fill_table},
    config::hooks::Hooks,
    helpers::{bold_cell, bytes_size_to_string, table},
    repository::CliRepo,
    status_err, Application, RUSTIC_APP,
};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::{anyhow, bail, Context, Result};
use clap::ValueHint;
use comfy_table::Cell;
use conflate::Merge;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use rustic_core::{
    BackupOptions, CommandInput, ConfigOptions, IndexedIds, KeyOptions, LocalSourceFilterOptions,
    LocalSourceSaveOptions, ParentOptions, PathList, ProgressBars, Repository, SnapshotOptions,
};

/// `backup` subcommand
#[serde_as]
#[derive(Clone, Command, Default, Debug, clap::Parser, Serialize, Deserialize, Merge)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
// Note: using cli_sources, sources and snapshots within this struct is a hack to support serde(deny_unknown_fields)
// for deserializing the backup options from TOML
// Unfortunately we cannot work with nested flattened structures, see
// https://github.com/serde-rs/serde/issues/1547
// A drawback is that a wrongly set "snapshots = ..." won't get correct error handling and need to be manually checked, see below.
#[allow(clippy::struct_excessive_bools)]
pub struct BackupCmd {
    /// Backup source (can be specified multiple times), use - for stdin. If no source is given, uses all
    /// sources defined in the config file
    #[clap(value_name = "SOURCE", value_hint = ValueHint::AnyPath)]
    #[merge(skip)]
    #[serde(skip)]
    cli_sources: Vec<String>,

    /// Set filename to be used when backing up from stdin
    #[clap(long, value_name = "FILENAME", default_value = "stdin", value_hint = ValueHint::FilePath)]
    #[merge(skip)]
    stdin_filename: String,

    /// Start the given command and use its output as stdin
    #[clap(long, value_name = "COMMAND")]
    #[merge(strategy=conflate::option::overwrite_none)]
    stdin_command: Option<CommandInput>,

    /// Manually set backup path in snapshot
    #[clap(long, value_name = "PATH", value_hint = ValueHint::DirPath)]
    #[merge(strategy=conflate::option::overwrite_none)]
    as_path: Option<PathBuf>,

    /// Ignore save options
    #[clap(flatten)]
    #[serde(flatten)]
    ignore_save_opts: LocalSourceSaveOptions,

    /// Don't scan the backup source for its size - this disables ETA estimation for backup.
    #[clap(long)]
    #[merge(strategy=conflate::bool::overwrite_false)]
    pub no_scan: bool,

    /// Output generated snapshot in json format
    #[clap(long)]
    #[merge(strategy=conflate::bool::overwrite_false)]
    json: bool,

    /// Show detailed information about generated snapshot
    #[clap(long, conflicts_with = "json")]
    #[merge(strategy=conflate::bool::overwrite_false)]
    long: bool,

    /// Don't show any output
    #[clap(long, conflicts_with_all = ["json", "long"])]
    #[merge(strategy=conflate::bool::overwrite_false)]
    quiet: bool,

    /// Initialize repository, if it doesn't exist yet
    #[clap(long)]
    #[merge(strategy=conflate::bool::overwrite_false)]
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

    /// Hooks to use
    #[clap(skip)]
    hooks: Hooks,

    /// Backup snapshots to generate
    #[clap(skip)]
    #[merge(strategy = merge_snapshots)]
    snapshots: Vec<BackupCmd>,

    /// Backup source, used within config file
    #[clap(skip)]
    #[merge(skip)]
    sources: Vec<String>,
}

/// Merge backup snapshots to generate
///
/// If a snapshot is already defined on left, use that. Else add it.
///
/// # Arguments
///
/// * `left` - Vector of backup sources
pub(crate) fn merge_snapshots(left: &mut Vec<BackupCmd>, mut right: Vec<BackupCmd>) {
    left.append(&mut right);
    left.sort_by(|opt1, opt2| opt1.sources.cmp(&opt2.sources));
    left.dedup_by(|opt1, opt2| opt1.sources == opt2.sources);
}

impl Runnable for BackupCmd {
    fn run(&self) {
        let config = RUSTIC_APP.config();

        // manually check for a "source" field, check is not done by serde, see above.
        if !config.backup.sources.is_empty() {
            status_err!("key \"sources\" is not valid in the [backup] section!");
            RUSTIC_APP.shutdown(Shutdown::Crash);
        }

        let snapshot_opts = &config.backup.snapshots;
        // manually check for a "sources" field, check is not done by serde, see above.
        if snapshot_opts.iter().any(|opt| !opt.snapshots.is_empty()) {
            status_err!("key \"snapshots\" is not valid in a [[backup.snapshots]] section!");
            RUSTIC_APP.shutdown(Shutdown::Crash);
        }
        if let Err(err) = config.repository.run(|repo| self.inner_run(repo)) {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl BackupCmd {
    fn inner_run(&self, repo: CliRepo) -> Result<()> {
        let config = RUSTIC_APP.config();
        let snapshot_opts = &config.backup.snapshots;

        // Initialize repository if --init is set and it is not yet initialized
        let repo = if self.init && repo.config_id()?.is_none() {
            if config.global.dry_run {
                bail!(
                    "cannot initialize repository {} in dry-run mode!",
                    repo.name
                );
            }
            init(repo.0, &self.key_opts, &self.config_opts)?
        } else {
            repo.open()?
        }
        .to_indexed_ids()?;

        let hooks = config.backup.hooks.with_context("backup");
        hooks.use_with(|| -> Result<_> {
            let config_snapshot_sources: Vec<_> = snapshot_opts
                .iter()
                .map(|opt| -> Result<_> {
                    Ok(PathList::from_iter(&opt.sources)
                        .sanitize()
                        .with_context(|| {
                            format!(
                                "error sanitizing sources=\"{:?}\" in config file",
                                opt.sources
                            )
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

            let snapshot_sources = match (self.cli_sources.is_empty(), snapshot_opts.is_empty()) {
                (false, _) => {
                    let item = PathList::from_iter(&self.cli_sources).sanitize()?;
                    vec![item]
                }
                (true, false) => {
                    info!("using all backup sources from config file.");
                    config_snapshot_sources.clone()
                }
                (true, true) => {
                    bail!("no backup source given.");
                }
            };
            if snapshot_sources.is_empty() {
                return Ok(());
            }

            let mut is_err = false;
            for sources in snapshot_sources {
                let mut opts = self.clone();

                // merge Options from config file, if given
                if let Some(idx) = config_snapshot_sources.iter().position(|s| s == &sources) {
                    info!("merging sources={sources} section from config file");
                    opts.merge(snapshot_opts[idx].clone());
                }
                if let Err(err) = opts.backup_snapshot(sources.clone(), &repo) {
                    error!("error backing up {sources}: {err}");
                    is_err = true;
                }
            }
            if is_err {
                Err(anyhow!("Not all snapshots were generated successfully!"))
            } else {
                Ok(())
            }
        })
    }

    fn backup_snapshot<P: ProgressBars, S: IndexedIds>(
        mut self,
        source: PathList,
        repo: &Repository<P, S>,
    ) -> Result<()> {
        let config = RUSTIC_APP.config();
        let snapshot_opts = &config.backup.snapshots;
        if let Some(path) = &self.as_path {
            // as_path only works in combination with a single target
            if source.len() > 1 {
                bail!("as-path only works with a single source!");
            }
            // merge Options from config file using as_path, if given
            if let Some(path) = path.as_os_str().to_str() {
                if let Some(idx) = snapshot_opts
                    .iter()
                    .position(|opt| opt.sources == vec![path])
                {
                    info!("merging snapshot=\"{path}\" section from config file");
                    self.merge(snapshot_opts[idx].clone());
                }
            }
        }

        // use the correct source-specific hooks
        let hooks = self.hooks.with_context(&format!("backup {source}"));

        // merge "backup" section from config file, if given
        self.merge(config.backup.clone());

        let backup_opts = BackupOptions::default()
            .stdin_filename(self.stdin_filename)
            .stdin_command(self.stdin_command)
            .as_path(self.as_path)
            .parent_opts(self.parent_opts)
            .ignore_save_opts(self.ignore_save_opts)
            .ignore_filter_opts(self.ignore_filter_opts)
            .no_scan(self.no_scan)
            .dry_run(config.global.dry_run);

        let snap = hooks.use_with(|| -> Result<_> {
            Ok(repo.backup(&backup_opts, &source, self.snap_opts.to_snapshot()?)?)
        })?;

        if self.json {
            let mut stdout = std::io::stdout();
            serde_json::to_writer_pretty(&mut stdout, &snap)?;
        } else if self.long {
            let mut table = table();

            let add_entry = |title: &str, value: String| {
                _ = table.add_row([bold_cell(title), Cell::new(value)]);
            };
            fill_table(&snap, add_entry);

            println!("{table}");
        } else if !self.quiet {
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
        Ok(())
    }
}
