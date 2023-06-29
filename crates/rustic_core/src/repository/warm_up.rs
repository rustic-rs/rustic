use std::process::Command;
use std::thread::sleep;

use log::{debug, warn};
use rayon::ThreadPoolBuilder;

use super::parse_command;
use crate::{
    error::RepositoryErrorKind, FileType, Id, Progress, ProgressBars, ReadBackend, Repository,
    RusticResult,
};

pub(super) mod constants {
    pub(super) const MAX_READER_THREADS_NUM: usize = 20;
}

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
        let commands = parse_command::<()>(&actual_command)
            .map_err(RepositoryErrorKind::FromNomError)?
            .1;
        let status = Command::new(commands[0]).args(&commands[1..]).status()?;
        if !status.success() {
            warn!("warm-up command was not successful for pack {pack:?}. {status}");
        }
    }
    p.finish();
    Ok(())
}

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
