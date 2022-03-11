use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::Parser;
use gethostname::gethostname;
use indicatif::{ProgressBar, ProgressStyle};
use path_absolutize::*;
use vlog::*;

use crate::archiver::{Archiver, Parent};
use crate::backend::{
    DecryptFullBackend, DryRunBackend, LocalSource, LocalSourceOptions, ReadSource,
};
use crate::index::IndexBackend;
use crate::repo::{ConfigFile, SnapshotFile};

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

pub(super) fn execute(opts: Opts, be: &impl DecryptFullBackend) -> Result<()> {
    let config = ConfigFile::from_backend_no_id(be)?;

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
        (false, None) => SnapshotFile::latest(&be, |snap| {
            OsString::from(&snap.hostname) == hostname && snap.paths.contains(&backup_path_str)
        })
        .ok(),
        (false, Some(parent)) => SnapshotFile::from_id(&be, &parent).ok(),
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

    let index = IndexBackend::only_full_trees(&be)?;

    let parent = Parent::new(&index, parent_tree.as_ref());
    let mut archiver = Archiver::new(be, index, poly, parent)?;

    let src = LocalSource::new(opts.ignore_opts, backup_path.to_path_buf())?;

    let p = if get_verbosity_level() == 1 {
        v1!("determining size of backup source...");
        ProgressBar::new(src.size()?).with_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {bytes:>10}/{total_bytes:10}"),
        )
    } else {
        ProgressBar::hidden()
    };

    v1!("starting backup...");
    for item in src {
        if let Err(e) = item.and_then(|(path, node)| {
            let size = *node.meta().size();
            archiver.add_entry(&path, node)?;
            p.inc(size);
            Ok(())
        }) {
            // TODO: Only ignore source errors, don't ignore repo errors
            eprintln!("ignoring error {}\n", e);
        }
    }
    p.finish_with_message("done");
    let mut snap = SnapshotFile::default();
    snap.set_paths(vec![backup_path.to_path_buf()]);
    snap.set_hostname(hostname);
    archiver.finalize_snapshot(snap)?;

    Ok(())
}
