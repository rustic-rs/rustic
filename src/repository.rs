//! Rustic Config
//!
//! See instructions in `commands.rs` to specify the path to your
//! application's configuration file and/or command-line options
//! for specifying it.

use std::fmt::Debug;
use std::ops::Deref;

use abscissa_core::Application;
use anyhow::{anyhow, bail, Result};
use clap::Parser;
use conflate::Merge;
use dialoguer::Password;
use rustic_backend::BackendOptions;
use rustic_core::{
    FullIndex, IndexedStatus, OpenStatus, ProgressBars, Repository, RepositoryOptions,
};
use serde::{Deserialize, Serialize};

use crate::config::progress_options::ProgressOptions;
use crate::config::Hooks;
use crate::RUSTIC_APP;

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
    fn repository<P>(&self, po: P) -> Result<RusticRepo<P>> {
        let backends = self.be.to_backends()?;
        let repo = Repository::new_with_progress(&self.repo, &backends, po)?;
        Ok(RusticRepo(repo))
    }

    pub fn run<T>(&self, f: impl FnOnce(CliRepo) -> Result<T>) -> Result<T> {
        let hooks = self.hooks.with_context("repository");
        let po = RUSTIC_APP.config().global.progress_options;
        hooks.use_with(|| f(self.repository(po)?))
    }

    pub fn run_open<T>(&self, f: impl FnOnce(CliOpenRepo) -> Result<T>) -> Result<T> {
        let hooks = self.hooks.with_context("repository");
        let po = RUSTIC_APP.config().global.progress_options;
        hooks.use_with(|| f(self.repository(po)?.open()?))
    }

    pub fn run_open_or_init_with<T: Clone>(
        &self,
        do_init: bool,
        init: impl FnOnce(CliRepo) -> Result<CliOpenRepo>,
        f: impl FnOnce(CliOpenRepo) -> Result<T>,
    ) -> Result<T> {
        let hooks = self.hooks.with_context("repository");
        let po = RUSTIC_APP.config().global.progress_options;
        hooks.use_with(|| {
            f(self
                .repository(po)?
                .open_or_init_repository_with(do_init, init)?)
        })
    }

    pub fn run_indexed_with_progress<P: Clone + ProgressBars, T>(
        &self,
        po: P,
        f: impl FnOnce(RusticIndexedRepo<P>) -> Result<T>,
    ) -> Result<T> {
        let hooks = self.hooks.with_context("repository");
        hooks.use_with(|| f(self.repository(po)?.indexed()?))
    }

    pub fn run_indexed<T>(&self, f: impl FnOnce(CliIndexedRepo) -> Result<T>) -> Result<T> {
        let po = RUSTIC_APP.config().global.progress_options;
        self.run_indexed_with_progress(po, f)
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
