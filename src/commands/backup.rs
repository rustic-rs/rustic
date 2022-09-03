use std::path::PathBuf;

use anyhow::{anyhow, Result};
use chrono::{Duration, Local};
use clap::{AppSettings, Parser};
use gethostname::gethostname;
use path_absolutize::*;
use vlog::*;

use super::{bytes, progress_bytes, progress_counter};
use crate::archiver::{Archiver, Parent};
use crate::backend::{
    DecryptFullBackend, DecryptWriteBackend, DryRunBackend, LocalSource, LocalSourceOptions,
    ReadSource,
};
use crate::blob::{Metadata, Node, NodeType};
use crate::index::IndexBackend;
use crate::repo::{ConfigFile, DeleteOption, SnapshotFile, SnapshotSummary, StringList};

#[derive(Parser)]
#[clap(global_setting(AppSettings::DeriveDisplayOrder))]
pub(super) struct Opts {
    /// Do not upload or write any data, just show what would be done
    #[clap(long, short = 'n')]
    dry_run: bool,

    /// Snapshot to use as parent
    #[clap(long, value_name = "SNAPSHOT", conflicts_with = "force")]
    parent: Option<String>,

    /// Use no parent, read all files
    #[clap(long, short, conflicts_with = "parent")]
    force: bool,

    /// Ignore ctime changes when checking for modified files
    #[clap(long, conflicts_with = "force")]
    ignore_ctime: bool,

    /// Ignore inode number changes when checking for modified files
    #[clap(long, conflicts_with = "force")]
    ignore_inode: bool,

    /// Tags to add to backup (can be specified multiple times)
    #[clap(long, value_name = "TAG[,TAG,..]")]
    tag: Vec<StringList>,

    /// Mark snapshot as uneraseable
    #[clap(long, conflicts_with = "delete-after")]
    delete_never: bool,

    /// Mark snapshot to be deleted after given duration (e.g. 10d)
    #[clap(long, value_name = "DURATION")]
    delete_after: Option<humantime::Duration>,

    /// Set filename to be used when backing up from stdin
    #[clap(long, value_name = "FILENAME", default_value = "stdin")]
    stdin_filename: String,

    #[clap(flatten)]
    ignore_opts: LocalSourceOptions,

    /// Backup source, use - for stdin
    #[clap(value_name = "SOURCE")]
    sources: Vec<String>,
}

pub(super) async fn execute(
    be: &impl DecryptFullBackend,
    opts: Opts,
    config: ConfigFile,
    command: String,
) -> Result<()> {
    let time = Local::now();
    let zstd = config.zstd()?;
    let mut be = DryRunBackend::new(be.clone(), opts.dry_run);
    be.set_zstd(zstd);

    if opts.sources.is_empty() {
        v1!("no backup source given.");
        return Ok(());
    }

    let index = IndexBackend::only_full_trees(&be.clone(), progress_counter()).await?;

    for source in opts.sources {
        v1!("\nbacking up {source}...");
        let be = be.clone();
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

        let hostname = gethostname();
        let hostname = hostname
            .to_str()
            .ok_or_else(|| anyhow!("non-unicode hostname {:?}", hostname))?
            .to_string();

        let parent = match (backup_stdin, opts.force, opts.parent.clone()) {
            (true, _, _) | (false, true, _) => None,
            (false, false, None) => SnapshotFile::latest(
                &be,
                |snap| snap.hostname == hostname && snap.paths.contains(&backup_path_str),
                progress_counter(),
            )
            .await
            .ok(),
            (false, false, Some(parent)) => SnapshotFile::from_id(&be, &parent).await.ok(),
        };

        let parent_tree = match &parent {
            Some(snap) => {
                v1!("using parent {}", snap.id);
                Some(snap.tree)
            }
            None => {
                v1!("using no parent");
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

        let parent = Parent::new(&index, parent_tree, opts.ignore_ctime, opts.ignore_inode).await;

        let snap = if backup_stdin {
            v1!("starting backup from stdin...");
            let mut archiver = Archiver::new(be, index, &config, parent, snap)?;
            let p = progress_bytes();
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
                    p,
                )
                .await?;

            archiver.finalize_snapshot().await?
        } else {
            let src = LocalSource::new(opts.ignore_opts.clone(), backup_path)?;

            let size = if get_verbosity_level() == 1 {
                v1!("determining size of backup source...");
                src.size()?
            } else {
                0
            };
            v1!("starting backup...");
            let mut archiver = Archiver::new(be, index.clone(), &config, parent, snap)?;
            let p = progress_bytes();
            p.set_length(size);
            for item in src {
                match item {
                    Err(e) => {
                        eprintln!("ignoring error {}\n", e)
                    }
                    Ok((path, node)) => {
                        if let Err(e) = archiver.add_entry(&path, node, p.clone()).await {
                            eprintln!("ignoring error {} for {:?}\n", e, path);
                        }
                    }
                }
            }
            p.finish_with_message("done");
            archiver.finalize_snapshot().await?
        };

        let summary = snap.summary.unwrap();

        v1!(
            "Files:       {} new, {} changed, {} unchanged",
            summary.files_new,
            summary.files_changed,
            summary.files_unmodified
        );
        v1!(
            "Dirs:        {} new, {} changed, {} unchanged",
            summary.dirs_new,
            summary.dirs_changed,
            summary.dirs_unmodified
        );
        v2!("Data Blobs:  {} new", summary.data_blobs);
        v2!("Tree Blobs:  {} new", summary.tree_blobs);
        v1!(
            "Added to the repo: {} (raw: {})",
            bytes(summary.data_added_packed),
            bytes(summary.data_added)
        );

        v1!(
            "processed {} files, {}",
            summary.total_files_processed,
            bytes(summary.total_bytes_processed)
        );
        v1!("snapshot {} successfully saved.", snap.id);

        v1!("backup of {source} done.");
    }

    Ok(())
}
