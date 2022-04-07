use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use bytesize::ByteSize;
use clap::Parser;
use gethostname::gethostname;
use path_absolutize::*;
use vlog::*;

use super::{progress_bytes, progress_counter};
use crate::archiver::{Archiver, Parent};
use crate::backend::{
    DecryptFullBackend, DryRunBackend, LocalSource, LocalSourceOptions, ReadSource,
};
use crate::index::IndexBackend;
use crate::repo::{ConfigFile, Id, SnapshotFile, StringList};

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

    #[clap(flatten)]
    ignore_opts: LocalSourceOptions,

    /// backup source
    source: String,
}

pub(super) async fn execute(
    be: &impl DecryptFullBackend,
    opts: Opts,
    command: String,
) -> Result<()> {
    let mut snap = SnapshotFile::default();
    snap.command = Some(command);

    let config: ConfigFile = be.get_file(&Id::default()).await?;
    let be = DryRunBackend::new(be.clone(), opts.dry_run);

    let poly = u64::from_str_radix(config.chunker_polynomial(), 16)?;
    let backup_path = PathBuf::from(&opts.source);
    let backup_path = backup_path.absolutize()?;
    let backup_path_str = backup_path
        .to_str()
        .ok_or_else(|| anyhow!("non-unicode path {:?}", backup_path))?
        .to_string();
    snap.paths.add(backup_path_str.clone());

    let hostname = gethostname();
    snap.hostname = hostname
        .to_str()
        .ok_or_else(|| anyhow!("non-unicode hostname {:?}", hostname))?
        .to_string();

    for tags in opts.tag {
        snap.tags.add_all(tags);
    }

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

    let src = LocalSource::new(opts.ignore_opts, backup_path.to_path_buf())?;

    let p = progress_bytes();
    let size = if get_verbosity_level() == 1 {
        v1!("determining size of backup source..."); // this is done in src.size() below
        src.size()?
    } else {
        0
    };

    v1!("starting backup...");
    let mut archiver = Archiver::new(be, index, poly, parent, snap)?;
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

    v1!(
        "Files:       {} new, {} changed, {} unchanged",
        snap.files_new.unwrap(),
        snap.files_changed.unwrap(),
        snap.files_unchanged.unwrap()
    );
    v1!(
        "Dirs:        {} new, {} changed, {} unchanged",
        snap.trees_new.unwrap(),
        snap.trees_changed.unwrap(),
        snap.trees_unchanged.unwrap()
    );
    v2!("Data Blobs:  {} new", snap.data_blobs_written.unwrap());
    v2!("Tree Blobs:  {} new", snap.tree_blobs_written.unwrap());
    v1!(
        "Added to the repo: {}",
        ByteSize(snap.data_added.unwrap()).to_string_as(true)
    );

    v1!(
        "processed {} nodes, {}",
        snap.node_count.unwrap(),
        ByteSize(snap.size.unwrap()).to_string_as(true)
    );
    v1!("snapshot {} successfully saved.", snap.id);

    Ok(())
}
