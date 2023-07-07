//! `ls` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{commands::open_repository, status_err, Application, RUSTIC_APP};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;

use rustic_core::TreeStreamerOptions;

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

        let repo = open_repository(&config)?.to_indexed()?;

        let node =
            repo.node_from_snapshot_path(&self.snap, |sn| config.snapshot_filter.matches(sn))?;

        let recursive = !self.snap.contains(':') || self.recursive;

        for item in repo.ls(&node, &self.streamer_opts, recursive)? {
            let (path, _) = item?;
            println!("{path:?} ");
        }

        Ok(())
    }
}
