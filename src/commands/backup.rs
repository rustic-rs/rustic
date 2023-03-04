use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{anyhow, Result};
use chrono::{Duration, Local};
use clap::{AppSettings, Parser};
use gethostname::gethostname;
use log::*;
use merge::Merge;
use path_dedot::ParseDot;
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};

use super::{bytes, progress_bytes, progress_counter, RusticConfig};
use crate::archiver::{Archiver, Parent};
use crate::backend::{DryRunBackend, LocalSource, LocalSourceOptions, ReadSource};
use crate::blob::{Metadata, Node, NodeType};
use crate::index::IndexBackend;
use crate::repofile::{
    DeleteOption, SnapshotFile, SnapshotGroup, SnapshotGroupCriterion, SnapshotSummary, StringList,
};
use crate::repository::OpenRepository;

#[serde_as]
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

    /// Label snapshot with given label
    #[clap(long, value_name = "LABEL")]
    label: Option<String>,

    /// Tags to add to backup (can be specified multiple times)
    #[clap(long, value_name = "TAG[,TAG,..]")]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[merge(strategy = merge::vec::overwrite_empty)]
    tag: Vec<StringList>,

    /// Add description to snapshot
    #[clap(long, value_name = "DESCRIPTION")]
    description: Option<String>,

    /// Add description to snapshot from file
    #[clap(long, value_name = "FILE", conflicts_with = "description")]
    description_from: Option<PathBuf>,

    /// Mark snapshot as uneraseable
    #[clap(long, conflicts_with = "delete-after")]
    #[merge(strategy = merge::bool::overwrite_false)]
    delete_never: bool,

    /// Mark snapshot to be deleted after given duration (e.g. 10d)
    #[clap(long, value_name = "DURATION")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    delete_after: Option<humantime::Duration>,

    /// Set filename to be used when backing up from stdin
    #[clap(long, value_name = "FILENAME", default_value = "stdin")]
    #[merge(skip)]
    stdin_filename: String,

    /// Manually set backup path in snapshot
    #[clap(long, value_name = "PATH")]
    as_path: Option<PathBuf>,

    /// Set the host name manually
    #[clap(long, value_name = "NAME")]
    host: Option<String>,

    #[clap(flatten)]
    #[serde(flatten)]
    ignore_opts: LocalSourceOptions,

    /// Backup source (can be specified multiple times), use - for stdin. If no source is given, uses all
    /// sources defined in the config file
    #[clap(value_name = "SOURCE")]
    #[merge(skip)]
    #[serde(skip)]
    sources: Vec<String>,

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

    let mut config_opts: Vec<Opts> = config_file.get("backup.sources")?;

    let sources = match (opts.sources.is_empty(), config_opts.is_empty()) {
        (false, _) => opts.sources.clone(),
        (true, false) => {
            info!("using all backup sources from config file.");
            config_opts.iter().map(|opt| opt.source.clone()).collect()
        }
        (true, true) => {
            warn!("no backup source given.");
            return Ok(());
        }
    };

    let index = IndexBackend::only_full_trees(&repo.dbe, progress_counter(""))?;

    for source in sources {
        let mut opts = opts.clone();

        // merge Options from config file, if given
        if let Some(idx) = config_opts.iter().position(|opt| opt.source == *source) {
            info!("merging source=\"{source}\" section from config file");
            opts.merge(config_opts.remove(idx));
        }
        // merge Options from config file using as_path, if given
        if let Some(path) = &opts.as_path {
            if let Some(path) = path.as_os_str().to_str() {
                if let Some(idx) = config_opts.iter().position(|opt| opt.source == path) {
                    info!("merging source=\"{path}\" section from config file");
                    opts.merge(config_opts.remove(idx));
                }
            }
        }
        // merge "backup" section from config file, if given
        config_file.merge_into("backup", &mut opts)?;

        let be = DryRunBackend::new(repo.dbe.clone(), opts.dry_run);
        info!("starting to backup \"{source}\"...");
        let index = index.clone();
        let backup_stdin = source == "-";
        let backup_path = if backup_stdin {
            PathBuf::from(&opts.stdin_filename)
        } else {
            PathBuf::from(&source).parse_dot()?.to_path_buf()
        };
        let as_path = match opts.as_path {
            None => None,
            Some(p) => Some(p.parse_dot()?.to_path_buf()),
        };
        let backup_path_str = as_path.as_ref().unwrap_or(&backup_path);
        let backup_path_str = backup_path_str
            .to_str()
            .ok_or_else(|| anyhow!("non-unicode path {:?}", backup_path_str))?
            .to_string();

        let hostname = match opts.host {
            Some(host) => host,
            None => {
                let hostname = gethostname();
                hostname
                    .to_str()
                    .ok_or_else(|| anyhow!("non-unicode hostname {:?}", hostname))?
                    .to_string()
            }
        };

        let delete = match (opts.delete_never, opts.delete_after) {
            (true, _) => DeleteOption::Never,
            (_, Some(d)) => DeleteOption::After(time + Duration::from_std(*d)?),
            (false, None) => DeleteOption::NotSet,
        };

        let mut snap = SnapshotFile {
            time,
            hostname,
            label: opts.label.unwrap_or_default(),
            delete,
            summary: Some(SnapshotSummary {
                command: command.clone(),
                ..Default::default()
            }),
            description: opts.description,
            ..Default::default()
        };

        // use description from description file if it is given
        if let Some(file) = opts.description_from {
            snap.description = Some(std::fs::read_to_string(file)?);
        }

        snap.paths.add(backup_path_str.clone());
        snap.set_tags(opts.tag.clone());

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
            let src = LocalSource::new(opts.ignore_opts.clone(), backup_path.clone())?;

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
                                .join(path.strip_prefix(&backup_path).unwrap())
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

        info!("backup of \"{source}\" done.");
    }

    Ok(())
}
