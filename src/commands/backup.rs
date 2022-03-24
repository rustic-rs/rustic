use std::ffi::OsString;
use std::path::PathBuf;

use super::{progress_bytes, progress_counter};
use anyhow::{anyhow, Result};
use clap::Parser;
use gethostname::gethostname;
use path_absolutize::*;
use vlog::*;

use crate::archiver::{Archiver, Parent};
use crate::backend::{
    DecryptFullBackend, DryRunBackend, LocalSource, LocalSourceOptions, ReadSource,
};
use crate::index::IndexBackend;
use crate::repo::{ConfigFile, Id, SnapshotFile};

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

    #[clap(flatten)]
    ignore_opts: LocalSourceOptions,

    /// backup source
    source: String,
}

pub(super) async fn execute(be: &impl DecryptFullBackend, opts: Opts) -> Result<()> {
    let config: ConfigFile = be.get_file(&Id::default()).await?;

    let be = DryRunBackend::new(be.clone(), opts.dry_run);

    let poly = u64::from_str_radix(config.chunker_polynomial(), 16)?;
    let backup_path = PathBuf::from(&opts.source);
    let backup_path = backup_path.absolutize()?;
    let backup_path_str = backup_path
        .to_str()
        .ok_or_else(|| anyhow!("non-unicode path {:?}", backup_path))?
        .to_string();

    let hostname = gethostname();
    let parent = match (opts.force, opts.parent) {
        (true, _) => None,
        (false, None) => SnapshotFile::latest(
            &be,
            |snap| {
                OsString::from(&snap.hostname) == hostname && snap.paths.contains(&backup_path_str)
            },
            progress_counter(),
        )
        .await
        .ok(),
        (false, Some(parent)) => SnapshotFile::from_id(&be, &parent).await.ok(),
    };
    let parent_tree = match parent {
        Some(snap) => {
            v1!("using parent {}", snap.id);
            Some(snap.tree)
        }
        None => {
            v1!("using no parent");
            None
        }
    };

    let index = IndexBackend::only_full_trees(&be, progress_counter()).await?;

    let parent = Parent::new(&index, parent_tree).await;
    let mut archiver = Archiver::new(be, index, poly, parent)?;
    let src = LocalSource::new(opts.ignore_opts, backup_path.to_path_buf())?;

    let p = progress_bytes();
    let size = if get_verbosity_level() == 1 {
        v1!("determining size of backup source..."); // this is done in src.size() below
        src.size()?
    } else {
        0
    };

    v1!("starting backup...");
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
    let mut snap = SnapshotFile::default();
    snap.set_paths(vec![backup_path.to_path_buf()]);
    snap.set_hostname(hostname);
    archiver.finalize_snapshot(snap).await?;

    Ok(())
}
