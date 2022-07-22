use std::path::PathBuf;

use anyhow::{anyhow, Result};
use bytesize::ByteSize;
use chrono::{Duration, Local};
use clap::Parser;
use gethostname::gethostname;
use path_absolutize::*;
use vlog::*;

use super::{bytes, progress_bytes, progress_counter};
use crate::archiver::{Archiver, Parent};
use crate::backend::{
    DecryptFullBackend, DecryptWriteBackend, DryRunBackend, LocalSource, LocalSourceOptions,
    ReadSource,
};
use crate::index::IndexBackend;
use crate::repo::{ConfigFile, DeleteOption, SnapshotFile, SnapshotSummary, StringList};

#[derive(Parser)]
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

    /// Tags to add to backup (can be specified multiple times)
    #[clap(long, value_name = "TAG[,TAG,..]")]
    tag: Vec<StringList>,

    /// Mark snapshot as uneraseable
    #[clap(long, conflicts_with = "delete-after")]
    delete_never: bool,

    /// Mark snapshot to be deleted after given duration (e.g. 10d)
    #[clap(long, value_name = "DURATION")]
    delete_after: Option<humantime::Duration>,

    /// Default packsize. rustic tries to always produce packs greater than this value.
    /// Note that for large repos, packs can get even larger. Does only apply to data packs.
    #[clap(long, value_name = "SIZE", default_value = "50M")]
    default_packsize: ByteSize,

    #[clap(flatten)]
    ignore_opts: LocalSourceOptions,

    /// backup source
    source: String,
}

pub(super) async fn execute(
    be: &impl DecryptFullBackend,
    opts: Opts,
    config: ConfigFile,
    command: String,
) -> Result<()> {
    let time = Local::now();
    let poly = config.poly()?;
    let zstd = config.zstd()?;
    let mut be = DryRunBackend::new(be.clone(), opts.dry_run);
    be.set_zstd(zstd);

    let backup_path = PathBuf::from(&opts.source);
    let backup_path = backup_path.absolutize()?;
    let backup_path_str = backup_path
        .to_str()
        .ok_or_else(|| anyhow!("non-unicode path {:?}", backup_path))?
        .to_string();

    let hostname = gethostname();
    let hostname = hostname
        .to_str()
        .ok_or_else(|| anyhow!("non-unicode hostname {:?}", hostname))?
        .to_string();

    let parent = match (opts.force, opts.parent) {
        (true, _) => None,
        (false, None) => SnapshotFile::latest(
            &be,
            |snap| snap.hostname == hostname && snap.paths.contains(&backup_path_str),
            progress_counter(),
        )
        .await
        .ok(),
        (false, Some(parent)) => SnapshotFile::from_id(&be, &parent).await.ok(),
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
            command,
            ..Default::default()
        }),
        ..Default::default()
    };
    snap.paths.add(backup_path_str.clone());
    snap.set_tags(opts.tag);

    let index = IndexBackend::only_full_trees(&be, progress_counter()).await?;

    let parent = Parent::new(&index, parent_tree).await;

    let src = LocalSource::new(opts.ignore_opts, backup_path.to_path_buf())?;

    let size = if get_verbosity_level() == 1 {
        v1!("determining size of backup source...");
        src.size()?
    } else {
        0
    };
    let default_packsize = opts.default_packsize.as_u64().try_into()?;

    v1!("starting backup...");
    let mut archiver = Archiver::new(be, index, poly, parent, snap, zstd, default_packsize)?;
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
    let snap = archiver.finalize_snapshot().await?;
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

    Ok(())
}
