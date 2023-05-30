//! `dump` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{
    commands::{get_repository, open_repository},
    status_err, Application, RUSTIC_APP,
};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::{bail, Result};

use std::{io::Write, path::Path};

use rustic_core::{BlobType, IndexBackend, IndexedBackend, NodeType, SnapshotFile, Tree};

/// `dump` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct DumpCmd {
    /// file from snapshot to dump
    #[clap(value_name = "SNAPSHOT[:PATH]")]
    snap: String,
}

impl Runnable for DumpCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl DumpCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();

        let repo = open_repository(get_repository(&config));

        let be = &repo.dbe;
        let progress_options = &config.global.progress_options;

        let (id, path) = self.snap.split_once(':').unwrap_or((&self.snap, ""));
        let snap = SnapshotFile::from_str(
            be,
            id,
            |sn| config.snapshot_filter.matches(sn),
            &progress_options.progress_counter(""),
        )?;
        let index = IndexBackend::new(be, progress_options.progress_counter(""))?;
        let node = Tree::node_from_path(&index, snap.tree, Path::new(path))?;

        if node.node_type != NodeType::File {
            bail!("dump only supports regular files!");
        }

        let mut stdout = std::io::stdout();
        for id in node.content.unwrap() {
            // TODO: cache blobs which are needed later
            let data = index.blob_from_backend(BlobType::Data, &id)?;
            stdout.write_all(&data)?;
        }

        Ok(())
    }
}
