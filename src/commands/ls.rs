//! `ls` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{
    commands::{get_repository, open_repository},
    status_err, Application, RUSTIC_APP,
};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;

use std::path::Path;

use rustic_core::{IndexBackend, NodeStreamer, SnapshotFile, Tree, TreeStreamerOptions};

/// `ls` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct LsCmd {
    /// Snapshot/path to list
    #[clap(value_name = "SNAPSHOT[:PATH]")]
    snap: String,

    /// recursively list the dir (default when no PATH is given)
    #[clap(long)]
    recursive: bool,

    #[clap(flatten)]
    streamer_opts: TreeStreamerOptions,
}

impl Runnable for LsCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl LsCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();
        let progress_options = &config.global.progress_options;

        let repo = open_repository(get_repository(&config));

        let be = &repo.dbe;
        let mut recursive = self.recursive;

        let (id, path) = self.snap.split_once(':').unwrap_or_else(|| {
            recursive = true;
            (&self.snap, "")
        });
        let snap = SnapshotFile::from_str(
            be,
            id,
            |sn| config.snapshot_filter.matches(sn),
            &progress_options.progress_counter(""),
        )?;
        let index = IndexBackend::new(be, progress_options.progress_counter(""))?;
        let node = Tree::node_from_path(&index, snap.tree, Path::new(path))?;

        if recursive {
            NodeStreamer::new_with_glob(index, &node, &self.streamer_opts)?.for_each(|item| {
                let (path, _) = match item {
                    Ok(it) => it,
                    Err(err) => {
                        status_err!("{}", err);
                        RUSTIC_APP.shutdown(Shutdown::Crash);
                    }
                };
                println!("{path:?} ");
            });
        } else if node.is_dir() {
            let tree = Tree::from_backend(&index, node.subtree.unwrap())?.nodes;
            for node in tree {
                println!("{:?} ", node.name());
            }
        } else {
            println!("{:?} ", node.name());
        }

        Ok(())
    }
}
