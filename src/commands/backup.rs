use std::path::PathBuf;

use anyhow::{anyhow, Result};
use chrono::{Duration, Local};
use clap::{AppSettings, Parser};
use gethostname::gethostname;
use log::*;
use merge::Merge;
use path_absolutize::*;
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};

use super::{bytes, progress_bytes, progress_counter, RusticConfig};
use crate::archiver::{Archiver, Parent};
use crate::backend::{
    DecryptFullBackend, DecryptWriteBackend, DryRunBackend, LocalSource, LocalSourceOptions,
    ReadSource,
};
use crate::blob::{Metadata, Node, NodeType};
use crate::index::IndexBackend;
use crate::repo::{ConfigFile, DeleteOption, SnapshotFile, SnapshotSummary, StringList};

#[serde_as]
#[derive(Clone, Default, Parser, Deserialize, Merge)]
#[clap(global_setting(AppSettings::DeriveDisplayOrder))]
#[serde(default, rename_all = "kebab-case")]
pub(super) struct Opts {
    /// Do not upload or write any data, just show what would be done
    #[clap(long, short = 'n')]
    #[merge(strategy = merge::bool::overwrite_false)]
    dry_run: bool,

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

    /// Tags to add to backup (can be specified multiple times)
    #[clap(long, value_name = "TAG[,TAG,..]")]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[merge(strategy = merge::vec::overwrite_empty)]
    tag: Vec<StringList>,

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

pub(super) async fn execute(
    be: &impl DecryptFullBackend,
    opts: Opts,
    config: ConfigFile,
    config_file: RusticConfig,
    command: String,
) -> Result<()> {
    let time = Local::now();

    let zstd = config.zstd()?;

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

    let index = IndexBackend::only_full_trees(&be.clone(), progress_counter("")).await?;

    for source in sources {
        let mut opts = opts.clone();

        // merge Options from config file, if given
        if let Some(idx) = config_opts.iter().position(|opt| opt.source == *source) {
            opts.merge(config_opts.remove(idx));
        }
        // merge "backup" section from config file, if given
        config_file.merge_into("backup", &mut opts)?;

        let mut be = DryRunBackend::new(be.clone(), opts.dry_run);
        be.set_zstd(zstd);
        info!("starting to backup \"{source}\"...");
        let index = index.clone();
        let backup_stdin = source == "-";
        let backup_path = if backup_stdin {
            PathBuf::from(&opts.stdin_filename)
        } else {
            PathBuf::from(&source).absolutize()?.to_path_buf()
        };
        let backup_path_str = backup_path
            .to_str()
            .ok_or_else(|| anyhow!("non-unicode path {:?}", backup_path))?
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

        let parent = match (backup_stdin, opts.force, opts.parent.clone()) {
            (true, _, _) | (false, true, _) => None,
            (false, false, None) => SnapshotFile::latest(
                &be,
                |snap| snap.hostname == hostname && snap.paths.contains(&backup_path_str),
                progress_counter(""),
            )
            .await
            .ok(),
            (false, false, Some(parent)) => SnapshotFile::from_id(&be, &parent).ok(),
        };

        let parent_tree = match &parent {
            Some(snap) => {
                info!("using parent {}", snap.id);
                Some(snap.tree)
            }
            None => {
                info!("using no parent");
                None
            }
        };

        let delete = match (opts.delete_never, opts.delete_after) {
            (true, _) => DeleteOption::Never,
            (_, Some(d)) => DeleteOption::After(time + Duration::from_std(*d)?),
            (false, None) => DeleteOption::NotSet,
        };

        let mut snap = SnapshotFile {
            time,
            parent: parent.map(|sn| sn.id),
            hostname,
            delete,
            summary: Some(SnapshotSummary {
                command: command.clone(),
                ..Default::default()
            }),
            ..Default::default()
        };
        snap.paths.add(backup_path_str.clone());
        snap.set_tags(opts.tag.clone());

        let parent = Parent::new(&index, parent_tree, opts.ignore_ctime, opts.ignore_inode);

        let snap = if backup_stdin {
            let mut archiver = Archiver::new(be, index, &config, parent, snap)?;
            let p = progress_bytes("starting backup from stdin...");
            archiver
                .backup_reader(
                    std::io::stdin(),
                    Node::new(
                        backup_path_str,
                        NodeType::File,
                        Metadata::default(),
                        None,
                        None,
                    ),
                    p.clone(),
                )
                .await?;

            let snap = archiver.finalize_snapshot().await?;
            p.finish_with_message("done");
            snap
        } else {
            let src = LocalSource::new(opts.ignore_opts.clone(), backup_path)?;

            let p = progress_bytes("determining size...");
            if !p.is_hidden() {
                let size = src.size()?;
                p.set_length(size);
            };
            p.set_prefix("backing up...");
            let mut archiver = Archiver::new(be, index.clone(), &config, parent, snap)?;
            for item in src {
                match item {
                    Err(e) => {
                        warn!("ignoring error {}\n", e)
                    }
                    Ok((path, node)) => {
                        if let Err(e) = archiver.add_entry(&path, node, p.clone()).await {
                            warn!("ignoring error {} for {:?}\n", e, path);
                        }
                    }
                }
            }
            let snap = archiver.finalize_snapshot().await?;
            p.finish_with_message("done");
            snap
        };

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

        info!("backup of \"{source}\" done.");
    }

    Ok(())
}
