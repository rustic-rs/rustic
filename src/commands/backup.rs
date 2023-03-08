use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{anyhow, bail, Result};
use chrono::Local;
use clap::{AppSettings, Parser};
use log::*;
use merge::Merge;
use path_dedot::ParseDot;
use serde::Deserialize;
use toml::Value;

use super::{bytes, progress_bytes, progress_counter, RusticConfig};
use crate::archiver::{Archiver, Parent};
use crate::backend::{DryRunBackend, LocalSource, LocalSourceOptions, ReadSource};
use crate::blob::{Metadata, Node, NodeType};
use crate::index::IndexBackend;
use crate::repofile::{
    PathList, SnapshotFile, SnapshotGroup, SnapshotGroupCriterion, SnapshotOptions,
};
use crate::repository::OpenRepository;

#[derive(Clone, Default, Parser, Deserialize, Merge)]
#[clap(global_setting(AppSettings::DeriveDisplayOrder))]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(super) struct Opts {
    /// Output generated snapshot in json format
    #[clap(long)]
    #[merge(strategy = merge::bool::overwrite_false)]
    json: bool,

    /// Do not upload or write any data, just show what would be done
    #[clap(long, short = 'n')]
    #[merge(strategy = merge::bool::overwrite_false)]
    dry_run: bool,

    /// Group snapshots by any combination of host,label,paths,tags to find a suitable parent (default: host,label,paths)
    #[clap(long, short = 'g', value_name = "CRITERION")]
    group_by: Option<SnapshotGroupCriterion>,

    /// Snapshot to use as parent
    #[clap(long, value_name = "SNAPSHOT", conflicts_with = "force")]
    parent: Option<String>,

    /// Use no parent, read all files
    #[clap(long, short, conflicts_with = "parent")]
    #[merge(strategy = merge::bool::overwrite_false)]
    force: bool,

    /// Ignore ctime changes when checking for modified files
    #[clap(long, conflicts_with = "force")]
    #[merge(strategy = merge::bool::overwrite_false)]
    ignore_ctime: bool,

    /// Ignore inode number changes when checking for modified files
    #[clap(long, conflicts_with = "force")]
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
    snap_opts: SnapshotOptions,

    #[clap(flatten)]
    #[serde(flatten)]
    ignore_opts: LocalSourceOptions,

    /// Backup source (can be specified multiple times), use - for stdin. If no source is given, uses all
    /// sources defined in the config file
    #[clap(value_name = "SOURCE")]
    #[merge(skip)]
    #[serde(skip)]
    cli_sources: Vec<String>,

    // This is a hack to support serde(deny_unknown_fields) for deserializing the backup options from TOML
    // while still being able to use [[backup.sources]] in the config file.
    // A drawback is that a unkowen "sources = ..." won't be bailed...
    // Note that unfortunately we cannot work with nested flattened structures, see
    // https://github.com/serde-rs/serde/issues/1547
    #[clap(skip)]
    #[merge(skip)]
    #[serde(rename = "sources")]
    config_sources: Option<Value>,

    /// Backup source, used within config file
    #[clap(skip)]
    #[merge(skip)]
    source: String,
}

pub(super) fn execute(
    repo: OpenRepository,
    opts: Opts,
    config_file: RusticConfig,
    command: String,
) -> Result<()> {
    let time = Local::now();

    let config_opts: Vec<Opts> = config_file.get("backup.sources")?;

    let config_sources: Vec<_> = config_opts
        .iter()
        .filter_map(|opt| match PathList::from_string(&opt.source) {
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

    let sources = match (opts.cli_sources.is_empty(), config_opts.is_empty()) {
        (false, _) => vec![PathList::from_strings(&opts.cli_sources)?],
        (true, false) => {
            info!("using all backup sources from config file.");
            config_sources.clone()
        }
        (true, true) => {
            warn!("no backup source given.");
            return Ok(());
        }
    };

    let index = IndexBackend::only_full_trees(&repo.dbe, progress_counter(""))?;

    for source in sources {
        let mut opts = opts.clone();
        let index = index.clone();
        let backup_stdin = source == PathList::from_string("-")?;
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
        config_file.merge_into("backup", &mut opts)?;

        let be = DryRunBackend::new(repo.dbe.clone(), opts.dry_run);
        info!("starting to backup {source}...");
        let as_path = match opts.as_path {
            None => None,
            Some(p) => Some(p.parse_dot()?.to_path_buf()),
        };
        let backup_path_str = match &as_path {
            Some(as_path) => vec![as_path],
            None => backup_path.iter().collect(),
        };
        let backup_path_str = backup_path_str
            .iter()
            .map(|p| {
                Ok(p.to_str()
                    .ok_or_else(|| anyhow!("non-unicode path {:?}", backup_path_str))?
                    .to_string())
            })
            .collect::<Result<Vec<_>>>()?
            .join("\n");

        let mut snap = SnapshotFile::new_from_options(opts.snap_opts, time, command.clone())?;

        snap.paths.add(backup_path_str.clone());

        // get suitable snapshot group from snapshot and opts.group_by. This is used to filter snapshots for the parent detection
        let group = SnapshotGroup::from_sn(
            &snap,
            &opts
                .group_by
                .unwrap_or_else(|| SnapshotGroupCriterion::from_str("host,label,paths").unwrap()),
        );

        let parent = match (backup_stdin, opts.force, opts.parent.clone()) {
            (true, _, _) | (false, true, _) => None,
            (false, false, None) => {
                SnapshotFile::latest(&be, |snap| snap.has_group(&group), progress_counter("")).ok()
            }
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

        let parent = Parent::new(&index, parent_tree, opts.ignore_ctime, opts.ignore_inode);

        let snap = if backup_stdin {
            let mut archiver = Archiver::new(be, index, &repo.config, parent, snap)?;
            let p = progress_bytes("starting backup from stdin...");
            archiver.backup_reader(
                std::io::stdin(),
                Node::new(
                    backup_path_str,
                    NodeType::File,
                    Metadata::default(),
                    None,
                    None,
                ),
                p.clone(),
            )?;

            let snap = archiver.finalize_snapshot()?;
            p.finish_with_message("done");
            snap
        } else {
            let src = LocalSource::new(opts.ignore_opts.clone(), &backup_path)?;

            let p = progress_bytes("determining size...");
            if !p.is_hidden() {
                let size = src.size()?;
                p.set_length(size);
            };
            p.set_prefix("backing up...");
            let mut archiver = Archiver::new(be, index.clone(), &repo.config, parent, snap)?;
            for item in src {
                match item {
                    Err(e) => {
                        warn!("ignoring error {}\n", e);
                    }
                    Ok((path, node)) => {
                        let snapshot_path = if let Some(as_path) = &as_path {
                            as_path
                                .clone()
                                .join(path.strip_prefix(&backup_path[0]).unwrap())
                        } else {
                            path.clone()
                        };
                        if let Err(e) = archiver.add_entry(&snapshot_path, &path, node, p.clone()) {
                            warn!("ignoring error {} for {:?}\n", e, path);
                        }
                    }
                }
            }
            let snap = archiver.finalize_snapshot()?;
            p.finish_with_message("done");
            snap
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
                bytes(summary.data_added_packed),
                bytes(summary.data_added)
            );

            println!(
                "processed {} files, {}",
                summary.total_files_processed,
                bytes(summary.total_bytes_processed)
            );
            println!("snapshot {} successfully saved.", snap.id);
        }

        info!("backup of {source} done.");
    }

    Ok(())
}
