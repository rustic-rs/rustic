//! `check` subcommand

use crate::{
    repository::{get_global_grouped_snapshots, OpenRepo},
    status_err, Application, RUSTIC_APP,
};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use rustic_core::{repofile::SnapshotFile, CheckOptions, Credentials};

/// `check` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct CheckCmd {
    /// Snapshots to check. If none is given, use filter options to filter from all snapshots
    ///
    /// Snapshots can be identified the following ways: "01a2b3c4" or "latest" or "latest~N" (N >= 0)
    #[clap(value_name = "ID")]
    ids: Vec<String>,

    /// Check options
    #[clap(flatten)]
    opts: CheckOptions,
}

impl Runnable for CheckCmd {
    fn run(&self) {
        let config = RUSTIC_APP.config();
        let repository = &config.repository;

        // A hot repository intentionally contains only metadata and tree packs;
        // data packs live in the cold repository. First preserve the usual
        // hot/cold consistency checks without reading data. Then create an
        // isolated cold-only repository for the pack-data check. Reuse the
        // validated master key so an interactive user is not prompted twice.
        let result = if self.opts.read_data && repository.be.repo_hot.is_some() {
            let mut cold_repository = repository.clone();
            cold_repository.be.repo_hot = None;
            repository.run(|repo| {
                let repo = repo.open(&repository.credential_opts)?;
                let credentials = Credentials::Masterkey(repo.key());
                let mut metadata_opts = self.opts;
                metadata_opts.read_data = false;
                self.inner_run_with_options(repo, metadata_opts)?;

                let cold_repo = cold_repository
                    .repository(config.global.progress_options)?
                    .0
                    .open(&credentials)?;
                self.inner_run(cold_repo)
            })
        } else {
            repository.run_open(|repo| self.inner_run(repo))
        };

        if let Err(err) = result {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl CheckCmd {
    fn inner_run(&self, repo: OpenRepo) -> Result<()> {
        self.inner_run_with_options(repo, self.opts)
    }

    fn inner_run_with_options(&self, repo: OpenRepo, opts: CheckOptions) -> Result<()> {
        let snaps: Vec<SnapshotFile> = get_global_grouped_snapshots(&repo, &self.ids)?.into();
        let trees = snaps.into_iter().map(|snap| snap.tree).collect();
        repo.check_with_trees(opts, trees)?.is_ok()?;
        Ok(())
    }
}
