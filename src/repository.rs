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
    CredentialOptions, Credentials, Grouped, IndexedFullStatus, IndexedIdsStatus, Open, OpenStatus,
    ProgressBars, Repository, RepositoryOptions, SnapshotGroupCriterion, repofile::SnapshotFile,
};
use serde::{Deserialize, Serialize};

use crate::{RUSTIC_APP, config::hooks::Hooks};

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

    /// Credential options
    #[clap(flatten, next_help_heading = "credential options")]
    #[serde(flatten)]
    pub credential_opts: CredentialOptions,

    /// Hooks
    #[clap(skip)]
    pub hooks: Hooks,
}

impl AllRepositoryOptions {
    pub fn repository(&self, po: impl ProgressBars) -> Result<Repo> {
        let backends = self.be.to_backends()?;
        let repo = Repository::new_with_progress(&self.repo, &backends, po)?;
        Ok(Repo(repo))
    }

    pub fn run_with_progress<T>(
        &self,
        po: impl ProgressBars,
        f: impl FnOnce(Repo) -> Result<T>,
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

    pub fn run<T>(&self, f: impl FnOnce(Repo) -> Result<T>) -> Result<T> {
        let po = RUSTIC_APP.config().global.progress_options;
        self.run_with_progress(po, f)
    }

    pub fn run_open<T>(&self, f: impl FnOnce(OpenRepo) -> Result<T>) -> Result<T> {
        self.run(|repo| f(repo.open(&self.credential_opts)?))
    }

    pub fn run_open_or_init_with<T: Clone>(
        &self,
        do_init: bool,
        init: impl FnOnce(Repo) -> Result<OpenRepo>,
        f: impl FnOnce(OpenRepo) -> Result<T>,
    ) -> Result<T> {
        self.run(|repo| {
            f(repo.open_or_init_repository_with(&self.credential_opts, do_init, init)?)
        })
    }

    pub fn run_indexed_with_progress<T>(
        &self,
        po: impl ProgressBars,
        f: impl FnOnce(IndexedRepo) -> Result<T>,
    ) -> Result<T> {
        self.run_with_progress(po, |repo| f(repo.indexed(&self.credential_opts)?))
    }

    pub fn run_indexed<T>(&self, f: impl FnOnce(IndexedRepo) -> Result<T>) -> Result<T> {
        self.run(|repo| f(repo.indexed(&self.credential_opts)?))
    }
}

pub type OpenRepo = Repository<OpenStatus>;
pub type IndexedRepo = Repository<IndexedFullStatus>;
pub type IndexedIdsRepo = Repository<IndexedIdsStatus>;

#[derive(Debug)]
pub struct Repo(pub Repository<()>);

impl Deref for Repo {
    type Target = Repository<()>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Repo {
    pub fn open(self, credential_opts: &CredentialOptions) -> Result<OpenRepo> {
        match credential_opts.credentials()? {
            // if credentials are given, directly open the repository and don't retry
            Some(credentials) => Ok(self.0.open(&credentials)?),
            None => {
                for _ in 0..constants::MAX_PASSWORD_RETRIES {
                    let pass = Password::new()
                        .with_prompt("enter repository password")
                        .allow_empty_password(true)
                        .interact()?;
                    match self
                        .0
                        .clone() // needed; else we move repo in a loop
                        .open(&Credentials::Password(pass))
                    {
                        Ok(repo) => return Ok(repo),
                        Err(err) if err.is_incorrect_password() => continue,
                        Err(err) => return Err(err.into()),
                    }
                }
                Err(anyhow!("incorrect password"))
            }
        }
    }

    fn open_or_init_repository_with(
        self,
        credential_opts: &CredentialOptions,
        do_init: bool,
        init: impl FnOnce(Self) -> Result<OpenRepo>,
    ) -> Result<OpenRepo> {
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
            self.open(credential_opts)?
        };
        Ok(repo)
    }

    fn indexed(self, credential_opts: &CredentialOptions) -> Result<IndexedRepo> {
        let open = self.open(credential_opts)?;
        let check_index = RUSTIC_APP.config().global.check_index;
        let repo = if check_index {
            open.to_indexed_checked()
        } else {
            open.to_indexed()
        }?;
        Ok(repo)
    }
}

// get snapshots from ids allowing `latest`, if empty use all snapshots respecting the filters.
pub fn get_snapots_from_ids<S: Open>(
    repo: &Repository<S>,
    ids: &[String],
) -> Result<Vec<SnapshotFile>> {
    let config = RUSTIC_APP.config();
    let snapshots = if ids.is_empty() {
        get_filtered_snapshots(repo)?
    } else {
        repo.get_snapshots_from_strs(ids, |sn| config.snapshot_filter.matches(sn))?
    };
    Ok(snapshots)
}

// get all snapshots respecting the filters
pub fn get_filtered_snapshots<S: Open>(repo: &Repository<S>) -> Result<Vec<SnapshotFile>> {
    let config = RUSTIC_APP.config();
    let mut snapshots = repo.get_matching_snapshots(|sn| config.snapshot_filter.matches(sn))?;
    config.snapshot_filter.post_process(&mut snapshots);
    Ok(snapshots)
}

pub fn get_global_grouped_snapshots<S: Open>(
    repo: &Repository<S>,
    ids: &[String],
) -> Result<Grouped<SnapshotFile>> {
    let config = RUSTIC_APP.config();
    get_grouped_snapshots(repo, config.global.group_by.unwrap_or_default(), ids)
}

pub fn get_grouped_snapshots<S: Open>(
    repo: &Repository<S>,
    group_by: SnapshotGroupCriterion,
    ids: &[String],
) -> Result<Grouped<SnapshotFile>> {
    let config = RUSTIC_APP.config();
    let snapshots = if ids.is_empty() {
        repo.get_matching_snapshots(|sn| config.snapshot_filter.matches(sn))?
    } else {
        repo.get_snapshots_from_strs(ids, |sn| config.snapshot_filter.matches(sn))?
    };
    let mut group = Grouped::from_items(snapshots, group_by);
    for group in &mut group.groups {
        config.snapshot_filter.post_process(&mut group.items);
    }

    Ok(group)
}
