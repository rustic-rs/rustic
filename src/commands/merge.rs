//! `merge` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{
    commands::{get_repository, open_repository},
    status_err, Application, RUSTIC_APP,
};
use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use log::info;

use chrono::Local;

use rustic_core::{
    merge_trees, BlobType, DecryptWriteBackend, FileType, Id, IndexBackend, Indexer, Node, Packer,
    PathList, ReadIndex, SnapshotFile, SnapshotOptions, Tree,
};

/// `merge` subcommand
#[derive(clap::Parser, Default, Command, Debug)]
pub(super) struct MergeCmd {
    /// Snapshots to merge. If none is given, use filter options to filter from all snapshots.
    #[clap(value_name = "ID")]
    ids: Vec<String>,

    /// Output generated snapshot in json format
    #[clap(long)]
    json: bool,

    /// Remove input snapshots after merging
    #[clap(long)]
    delete: bool,

    #[clap(flatten, next_help_heading = "Snapshot options")]
    snap_opts: SnapshotOptions,
}

impl Runnable for MergeCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl MergeCmd {
    fn inner_run(&self) -> Result<()> {
        let now = Local::now();

        let command: String = std::env::args_os()
            .map(|s| s.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(" ");

        let config = RUSTIC_APP.config();
        let progress_options = &config.global.progress_options;

        let repo = open_repository(get_repository(&config));

        let be = &repo.dbe;

        let snapshots = if self.ids.is_empty() {
            SnapshotFile::all_from_backend(be, |sn| config.snapshot_filter.matches(sn))?
        } else {
            SnapshotFile::from_ids(be, &self.ids)?
        };
        let index =
            IndexBackend::only_full_trees(&be.clone(), progress_options.progress_counter(""))?;

        let indexer = Indexer::new(be.clone()).into_shared();
        let packer = Packer::new(
            be.clone(),
            BlobType::Tree,
            indexer.clone(),
            &repo.config,
            index.total_size(BlobType::Tree),
        )?;

        let mut snap = SnapshotFile::new_from_options(&self.snap_opts, now, command)?;

        let paths =
            PathList::from_strings(snapshots.iter().flat_map(|snap| snap.paths.iter()), false)?;
        snap.paths.set_paths(&paths.paths())?;

        // set snapshot time to time of latest snapshot to be merged
        snap.time = snapshots
            .iter()
            .max_by_key(|sn| sn.time)
            .map_or(now, |sn| sn.time);

        let mut summary = snap.summary.take().unwrap();
        summary.backup_start = Local::now();

        let p = progress_options.progress_spinner("merging snapshots...");
        let trees: Vec<Id> = snapshots.iter().map(|sn| sn.tree).collect();

        let cmp = |n1: &Node, n2: &Node| n1.meta.mtime.cmp(&n2.meta.mtime);
        let save = |tree: Tree| {
            let (chunk, new_id) = tree.serialize()?;
            let size = match u64::try_from(chunk.len()) {
                Ok(it) => it,
                Err(err) => {
                    status_err!("{}", err);
                    RUSTIC_APP.shutdown(Shutdown::Crash);
                }
            };
            if !index.has_tree(&new_id) {
                packer.add(chunk.into(), new_id)?;
            }
            Ok((new_id, size))
        };

        let tree_merged = merge_trees(&index, &trees, &cmp, &save, &mut summary)?;
        snap.tree = tree_merged;

        let stats = packer.finalize()?;
        stats.apply(&mut summary, BlobType::Tree);

        indexer.write().unwrap().finalize()?;

        p.finish();

        summary.finalize(now)?;
        snap.summary = Some(summary);

        let new_id = be.save_file(&snap)?;
        snap.id = new_id;

        if self.json {
            let mut stdout = std::io::stdout();
            serde_json::to_writer_pretty(&mut stdout, &snap)?;
        }
        info!("saved new snapshot as {new_id}.");

        if self.delete {
            let now = Local::now();
            let p = progress_options.progress_counter("deleting old snapshots...");
            let snap_ids: Vec<_> = snapshots
                .iter()
                .filter(|sn| !sn.must_keep(now))
                .map(|sn| sn.id)
                .collect();
            be.delete_list(FileType::Snapshot, true, snap_ids.iter(), p)?;
        }

        Ok(())
    }
}
