//! `repoinfo` subcommand

use crate::{Application, RUSTIC_APP, jinja::output_repoinfo, repository::Repo, status_err};

use abscissa_core::{Command, Runnable, Shutdown};
use serde::Serialize;

use anyhow::Result;
use rustic_core::{IndexInfos, RepoFileInfos};

/// `repoinfo` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct RepoInfoCmd {
    /// Only scan repository files (doesn't need credentials)
    #[clap(long)]
    only_files: bool,

    /// Only scan index
    #[clap(long)]
    only_index: bool,

    /// Show infos in json format
    #[clap(long)]
    json: bool,
}

impl Runnable for RepoInfoCmd {
    fn run(&self) {
        if let Err(err) = RUSTIC_APP
            .config()
            .repository
            .run(|repo| self.inner_run(repo))
        {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

/// Infos about the repository
///
/// This struct is used to serialize infos in `json` format.
#[serde_with::apply(Option => #[serde(default, skip_serializing_if = "Option::is_none")])]
#[derive(Serialize)]
pub struct RepoInfos {
    files: Option<RepoFileInfos>,
    index: Option<IndexInfos>,
}

impl RepoInfoCmd {
    fn inner_run(&self, repo: Repo) -> Result<()> {
        let config = RUSTIC_APP.config();
        let infos = RepoInfos {
            files: (!self.only_index)
                .then(|| -> Result<_> { Ok(repo.infos_files()?) })
                .transpose()?,
            index: (!self.only_files)
                .then(|| -> Result<_> {
                    Ok(repo
                        .open(&config.repository.credential_opts)?
                        .infos_index()?)
                })
                .transpose()?,
        };

        if self.json {
            let mut stdout = std::io::stdout();
            serde_json::to_writer_pretty(&mut stdout, &infos)?;
        } else {
            let output = output_repoinfo(&infos)?;
            println!("{output}");
        }

        Ok(())
    }
}
