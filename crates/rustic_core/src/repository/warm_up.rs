use std::process::Command;
use std::thread::sleep;

use log::{debug, warn};
use rayon::ThreadPoolBuilder;
use shell_words::split;

use crate::{
    backend::{FileType, ReadBackend},
    error::{RepositoryErrorKind, RusticResult},
    id::Id,
    progress::{Progress, ProgressBars},
    repository::Repository,
};

pub(super) mod constants {
    /// The maximum number of reader threads to use for warm-up.
    pub(super) const MAX_READER_THREADS_NUM: usize = 20;
}

/// Warm up the repository and wait.
///
/// # Arguments
///
/// * `repo` - The repository to warm up.
/// * `packs` - The packs to warm up.
///
/// # Errors
///
/// * [`RepositoryErrorKind::FromSplitError`] - If the command could not be parsed.
/// * [`RepositoryErrorKind::FromThreadPoolbilderError`] - If the thread pool could not be created.
///
/// [`RepositoryErrorKind::FromSplitError`]: crate::error::RepositoryErrorKind::FromSplitError
/// [`RepositoryErrorKind::FromThreadPoolbilderError`]: crate::error::RepositoryErrorKind::FromThreadPoolbilderError
pub(crate) fn warm_up_wait<P: ProgressBars, S>(
    repo: &Repository<P, S>,
    packs: impl ExactSizeIterator<Item = Id>,
) -> RusticResult<()> {
    warm_up(repo, packs)?;
    if let Some(wait) = repo.opts.warm_up_wait {
        let p = repo.pb.progress_spinner(format!("waiting {wait}..."));
        sleep(*wait);
        p.finish();
    }
    Ok(())
}

/// Warm up the repository.
///
/// # Arguments
///
/// * `repo` - The repository to warm up.
/// * `packs` - The packs to warm up.
///
/// # Errors
///
/// * [`RepositoryErrorKind::FromSplitError`] - If the command could not be parsed.
/// * [`RepositoryErrorKind::FromThreadPoolbilderError`] - If the thread pool could not be created.
///
/// [`RepositoryErrorKind::FromSplitError`]: crate::error::RepositoryErrorKind::FromSplitError
/// [`RepositoryErrorKind::FromThreadPoolbilderError`]: crate::error::RepositoryErrorKind::FromThreadPoolbilderError
pub(crate) fn warm_up<P: ProgressBars, S>(
    repo: &Repository<P, S>,
    packs: impl ExactSizeIterator<Item = Id>,
) -> RusticResult<()> {
    if let Some(command) = &repo.opts.warm_up_command {
        warm_up_command(packs, command, &repo.pb)?;
    } else if repo.opts.warm_up {
        warm_up_access(repo, packs)?;
    }
    Ok(())
}

/// Warm up the repository using a command.
///
/// # Arguments
///
/// * `packs` - The packs to warm up.
/// * `command` - The command to execute.
/// * `pb` - The progress bar to use.
///
/// # Errors
///
/// * [`RepositoryErrorKind::FromSplitError`] - If the command could not be parsed.
///
/// [`RepositoryErrorKind::FromSplitError`]: crate::error::RepositoryErrorKind::FromSplitError
fn warm_up_command<P: ProgressBars>(
    packs: impl ExactSizeIterator<Item = Id>,
    command: &str,
    pb: &P,
) -> RusticResult<()> {
    let p = pb.progress_counter("warming up packs...");
    p.set_length(packs.len() as u64);
    for pack in packs {
        let actual_command = command.replace("%id", &pack.to_hex());
        debug!("calling {actual_command}...");
        let commands = split(&actual_command).map_err(RepositoryErrorKind::FromSplitError)?;
        let status = Command::new(&commands[0]).args(&commands[1..]).status()?;
        if !status.success() {
            warn!("warm-up command was not successful for pack {pack:?}. {status}");
        }
    }
    p.finish();
    Ok(())
}

/// Warm up the repository using access.
///
/// # Arguments
///
/// * `repo` - The repository to warm up.
/// * `packs` - The packs to warm up.
///
/// # Errors
///
/// * [`RepositoryErrorKind::FromThreadPoolbilderError`] - If the thread pool could not be created.
///
/// [`RepositoryErrorKind::FromThreadPoolbilderError`]: crate::error::RepositoryErrorKind::FromThreadPoolbilderError
fn warm_up_access<P: ProgressBars, S>(
    repo: &Repository<P, S>,
    packs: impl ExactSizeIterator<Item = Id>,
) -> RusticResult<()> {
    let mut be = repo.be.clone();
    be.set_option("retry", "false")?;

    let p = repo.pb.progress_counter("warming up packs...");
    p.set_length(packs.len() as u64);

    let pool = ThreadPoolBuilder::new()
        .num_threads(constants::MAX_READER_THREADS_NUM)
        .build()
        .map_err(RepositoryErrorKind::FromThreadPoolbilderError)?;
    let p = &p;
    let be = &be;
    pool.in_place_scope(|s| {
        for pack in packs {
            s.spawn(move |_| {
                // ignore errors as they are expected from the warm-up
                _ = be.read_partial(FileType::Pack, &pack, false, 0, 1);
                p.inc(1);
            });
        }
    });

    p.finish();

    Ok(())
}
