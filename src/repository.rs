//! Rustic Config
//!
//! See instructions in `commands.rs` to specify the path to your
//! application's configuration file and/or command-line options
//! for specifying it.

use std::collections::HashMap;
use std::fmt::Debug;
use std::ops::Deref;

use abscissa_core::Application;
use anyhow::{Result, anyhow, bail};
use clap::Parser;
use conflate::Merge;
use dialoguer::Password;
use rustic_backend::BackendOptions;
use rustic_core::{
    FullIndex, IndexedStatus, Open, OpenStatus, ProgressBars, Repository, RepositoryOptions,
    SnapshotGroup, SnapshotGroupCriterion, repofile::SnapshotFile,
};
use serde::{Deserialize, Serialize};

use crate::{
    RUSTIC_APP,
    config::{hooks::Hooks, progress_options::ProgressOptions},
};

pub(super) mod constants {
    pub(super) const MAX_PASSWORD_RETRIES: usize = 5;
}

#[derive(Clone, Default, Debug, Parser, Serialize, Deserialize, Merge)]
#[serde(default, rename_all = "kebab-case")]
pub struct AllRepositoryOptions {
    /// Backend options
    #[clap(flatten)]
    #[serde(flatten)]
    pub be: BackendOptions,

    /// Repository options
    #[clap(flatten)]
    #[serde(flatten)]
    pub repo: RepositoryOptions,

    /// Hooks
    #[clap(skip)]
    pub hooks: Hooks,
}

pub type CliRepo = RusticRepo<ProgressOptions>;
pub type CliOpenRepo = Repository<ProgressOptions, OpenStatus>;
pub type RusticIndexedRepo<P> = Repository<P, IndexedStatus<FullIndex, OpenStatus>>;
pub type CliIndexedRepo = RusticIndexedRepo<ProgressOptions>;

impl AllRepositoryOptions {
    pub fn repository<P>(&self, po: P) -> Result<RusticRepo<P>> {
        let backends = self.be.to_backends()?;
        let repo = Repository::new_with_progress(&self.repo, &backends, po)?;
        Ok(RusticRepo(repo))
    }

    pub fn run_with_progress<P: Clone + ProgressBars, T>(
        &self,
        po: P,
        f: impl FnOnce(RusticRepo<P>) -> Result<T>,
    ) -> Result<T> {
        let hooks = self
            .hooks
            .with_env(&HashMap::from([(
                "RUSTIC_ACTION".to_string(),
                "repository".to_string(),
            )]))
            .with_context("repository");
        hooks.use_with(|| f(self.repository(po)?))
    }

    pub fn run<T>(&self, f: impl FnOnce(CliRepo) -> Result<T>) -> Result<T> {
        let po = RUSTIC_APP.config().global.progress_options;
        self.run_with_progress(po, f)
    }

    pub fn run_open<T>(&self, f: impl FnOnce(CliOpenRepo) -> Result<T>) -> Result<T> {
        self.run(|repo| f(repo.open()?))
    }

    pub fn run_open_or_init_with<T: Clone>(
        &self,
        do_init: bool,
        init: impl FnOnce(CliRepo) -> Result<CliOpenRepo>,
        f: impl FnOnce(CliOpenRepo) -> Result<T>,
    ) -> Result<T> {
        self.run(|repo| f(repo.open_or_init_repository_with(do_init, init)?))
    }

    pub fn run_indexed_with_progress<P: Clone + ProgressBars, T>(
        &self,
        po: P,
        f: impl FnOnce(RusticIndexedRepo<P>) -> Result<T>,
    ) -> Result<T> {
        self.run_with_progress(po, |repo| f(repo.indexed()?))
    }

    pub fn run_indexed<T>(&self, f: impl FnOnce(CliIndexedRepo) -> Result<T>) -> Result<T> {
        self.run(|repo| f(repo.indexed()?))
    }
}

#[derive(Debug)]
pub struct RusticRepo<P>(pub Repository<P, ()>);

impl<P> Deref for RusticRepo<P> {
    type Target = Repository<P, ()>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<P: Clone + ProgressBars> RusticRepo<P> {
    pub fn open(self) -> Result<Repository<P, OpenStatus>> {
        match self.0.password()? {
            // if password is given, directly return the result of find_key_in_backend and don't retry
            Some(pass) => {
                return Ok(self.0.open_with_password(&pass)?);
            }
            None => {
                for _ in 0..constants::MAX_PASSWORD_RETRIES {
                    let pass = Password::new()
                        .with_prompt("enter repository password")
                        .allow_empty_password(true)
                        .interact()?;
                    match self.0.clone().open_with_password(&pass) {
                        Ok(repo) => return Ok(repo),
                        Err(err) if err.is_incorrect_password() => continue,
                        Err(err) => return Err(err.into()),
                    }
                }
            }
        }
        Err(anyhow!("incorrect password"))
    }

    fn open_or_init_repository_with(
        self,
        do_init: bool,
        init: impl FnOnce(Self) -> Result<Repository<P, OpenStatus>>,
    ) -> Result<Repository<P, OpenStatus>> {
        let dry_run = RUSTIC_APP.config().global.check_index;
        // Initialize repository if --init is set and it is not yet initialized
        let repo = if do_init && self.0.config_id()?.is_none() {
            if dry_run {
                bail!(
                    "cannot initialize repository {} in dry-run mode!",
                    self.0.name
                );
            }
            init(self)?
        } else {
            self.open()?
        };
        Ok(repo)
    }

    fn indexed(self) -> Result<Repository<P, IndexedStatus<FullIndex, OpenStatus>>> {
        let open = self.open()?;
        let check_index = RUSTIC_APP.config().global.check_index;
        let repo = if check_index {
            open.to_indexed_checked()
        } else {
            open.to_indexed()
        }?;
        Ok(repo)
    }
}

pub fn get_filtered_snapshots<P: ProgressBars, S: Open>(
    repo: &Repository<P, S>,
) -> Result<Vec<SnapshotFile>> {
    let config = RUSTIC_APP.config();
    let mut snapshots = repo.get_matching_snapshots(|sn| config.snapshot_filter.matches(sn))?;
    config.snapshot_filter.post_process(&mut snapshots);
    Ok(snapshots)
}

pub fn get_global_grouped_snapshots<P: ProgressBars, S: Open>(
    repo: &Repository<P, S>,
    ids: &[String],
) -> Result<Vec<(SnapshotGroup, Vec<SnapshotFile>)>> {
    let config = RUSTIC_APP.config();
    get_grouped_snapshots(repo, config.global.group_by.unwrap_or_default(), ids)
}

pub fn get_grouped_snapshots<P: ProgressBars, S: Open>(
    repo: &Repository<P, S>,
    group_by: SnapshotGroupCriterion,
    ids: &[String],
) -> Result<Vec<(SnapshotGroup, Vec<SnapshotFile>)>> {
    let config = RUSTIC_APP.config();
    let mut groups =
        repo.get_snapshot_group(ids, group_by, |sn| config.snapshot_filter.matches(sn))?;

    for (_, snaps) in &mut groups {
        config.snapshot_filter.post_process(snaps);
    }
    Ok(groups)
}
