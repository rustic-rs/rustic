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
use reqwest::Url;
use rustic_backend::BackendOptions;
use rustic_core::{
    CredentialOptions, Credentials, Grouped, IndexedFullStatus, IndexedIdsStatus, Open, OpenStatus,
    ProgressBars, Repository, RepositoryOptions, RusticResult, SnapshotGroupCriterion,
    repofile::SnapshotFile,
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
        let backends = self.backend_options().to_backends()?;
        let repo = Repository::new_with_progress(&self.repo, &backends, po)?;
        Ok(Repo(repo))
    }

    fn backend_options(&self) -> BackendOptions {
        let mut options = self.be.clone();

        let rest_username = std::env::var("RESTIC_REST_USERNAME").ok();
        let rest_password = std::env::var("RESTIC_REST_PASSWORD").ok();
        apply_rest_environment_credentials(
            &mut options.repository,
            rest_username.as_deref(),
            rest_password.as_deref(),
        );
        apply_rest_environment_credentials(
            &mut options.repo_hot,
            rest_username.as_deref(),
            rest_password.as_deref(),
        );

        options
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

/// Apply restic-compatible REST credentials without storing them in the config.
///
/// Like restic, credentials embedded in a repository URL take precedence over
/// environment variables. A value is only injected when at least one of the
/// two environment variables is set; an omitted counterpart is treated as an
/// empty string.
fn apply_rest_environment_credentials(
    repository: &mut Option<String>,
    username: Option<&str>,
    password: Option<&str>,
) {
    let Some(rest_url) = repository
        .as_deref()
        .and_then(|repository| repository.strip_prefix("rest:"))
    else {
        return;
    };

    let Ok(mut url) = Url::parse(rest_url) else {
        // Let the backend report invalid repository URLs using its usual
        // diagnostics instead of replacing that error with environment handling.
        return;
    };

    // Match restic's precedence: an explicit username or password in the URL
    // takes precedence over RESTIC_REST_USERNAME and RESTIC_REST_PASSWORD.
    if !url.username().is_empty()
        || url.password().is_some()
        || (username.is_none() && password.is_none())
    {
        return;
    }

    if url.set_username(username.unwrap_or_default()).is_ok()
        && url.set_password(Some(password.unwrap_or_default())).is_ok()
    {
        *repository = Some(format!("rest:{url}"));
    }
}

#[cfg(test)]
mod tests {
    use reqwest::Url;

    use super::apply_rest_environment_credentials;

    fn url_from_repository(repository: &Option<String>) -> Url {
        Url::parse(
            repository
                .as_deref()
                .unwrap()
                .strip_prefix("rest:")
                .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn adds_restic_environment_credentials_to_a_rest_url_without_userinfo() {
        let mut repository = Some("rest:https://example.invalid/repository".to_string());

        apply_rest_environment_credentials(
            &mut repository,
            Some("restic user"),
            Some("password:@/ with spaces"),
        );

        let url = url_from_repository(&repository);
        assert_eq!(url.username(), "restic%20user");
        assert_eq!(url.password(), Some("password%3A%40%2F%20with%20spaces"));
        assert_eq!(url.host_str(), Some("example.invalid"));
        assert_eq!(url.path(), "/repository");
    }

    #[test]
    fn rest_url_credentials_take_precedence_over_environment_credentials() {
        let mut repository =
            Some("rest:https://url-user:url-password@example.invalid/repository".to_string());

        apply_rest_environment_credentials(&mut repository, Some("env-user"), Some("env-pass"));

        let url = url_from_repository(&repository);
        assert_eq!(url.username(), "url-user");
        assert_eq!(url.password(), Some("url-password"));
    }

    #[test]
    fn partial_rest_url_credentials_take_precedence_over_environment_credentials() {
        let mut repository = Some("rest:https://url-user@example.invalid/repository".to_string());

        apply_rest_environment_credentials(&mut repository, Some("env-user"), Some("env-pass"));

        let url = url_from_repository(&repository);
        assert_eq!(url.username(), "url-user");
        assert_eq!(url.password(), None);
    }

    #[test]
    fn leaves_rest_url_unchanged_when_no_environment_credentials_are_set() {
        let original = "rest:https://example.invalid/repository".to_string();
        let mut repository = Some(original.clone());

        apply_rest_environment_credentials(&mut repository, None, None);

        assert_eq!(repository, Some(original));
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
    pub fn open_with(
        self,
        credential_opts: &CredentialOptions,
        open: impl Fn(Repository<()>, &Credentials) -> RusticResult<OpenRepo>,
    ) -> Result<OpenRepo> {
        match credential_opts.credentials()? {
            // if credentials are given, directly open the repository and don't retry
            Some(credentials) => Ok(open(self.0, &credentials)?),
            None => {
                for _ in 0..constants::MAX_PASSWORD_RETRIES {
                    let pass = Password::new()
                        .with_prompt("enter repository password")
                        .allow_empty_password(true)
                        .interact()?;
                    match open(self.0.clone(), &Credentials::Password(pass)) {
                        Ok(repo) => return Ok(repo),
                        Err(err) if err.is_incorrect_password() => continue,
                        Err(err) => return Err(err.into()),
                    }
                }
                Err(anyhow!("incorrect password"))
            }
        }
    }
    pub fn open(self, credential_opts: &CredentialOptions) -> Result<OpenRepo> {
        self.open_with(credential_opts, |repo, credentials| repo.open(credentials))
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
