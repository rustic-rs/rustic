//! `repoinfo` subcommand

use crate::{
    Application, RUSTIC_APP,
    helpers::{bytes_size_to_string, table_right_from},
    repository::Repo,
    status_err,
};

use abscissa_core::{Command, Runnable, Shutdown};
use serde::Serialize;

use anyhow::Result;
use rustic_core::{IndexInfos, RepoFileInfo, RepoFileInfos};

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
struct Infos {
    files: Option<RepoFileInfos>,
    index: Option<IndexInfos>,
}

impl RepoInfoCmd {
    fn inner_run(&self, repo: Repo) -> Result<()> {
        let config = RUSTIC_APP.config();
        let infos = Infos {
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
            return Ok(());
        }

        if let Some(file_info) = infos.files {
            print_file_info("repository files", file_info.repo);
            if let Some(info) = file_info.repo_hot {
                print_file_info("hot repository files", info);
            }
        }

        if let Some(index_info) = infos.index {
            print_index_info(index_info);
        }
        Ok(())
    }
}

/// Print infos about repository files
///
/// # Arguments
///
/// * `text` - the text to print before the table
/// * `info` - the [`RepoFileInfo`]s to print
pub fn print_file_info(text: &str, info: Vec<RepoFileInfo>) {
    let mut table = table_right_from(1, ["File type", "Count", "Total Size"]);
    let mut total_count = 0;
    let mut total_size = 0;
    for row in info {
        _ = table.add_row([
            format!("{:?}", row.tpe),
            row.count.to_string(),
            bytes_size_to_string(row.size),
        ]);
        total_count += row.count;
        total_size += row.size;
    }
    println!("{text}");
    _ = table.add_row([
        "Total".to_string(),
        total_count.to_string(),
        bytes_size_to_string(total_size),
    ]);

    println!();
    println!("{table}");
    println!();
}

/// Print infos about index
///
/// # Arguments
///
/// * `index_info` - the [`IndexInfos`] to print
pub fn print_index_info(index_info: IndexInfos) {
    let mut table = table_right_from(
        1,
        ["Blob type", "Count", "Total Size", "Total Size in Packs"],
    );

    let mut total_count = 0;
    let mut total_data_size = 0;
    let mut total_size = 0;

    for blobs in &index_info.blobs {
        _ = table.add_row([
            format!("{:?}", blobs.blob_type),
            blobs.count.to_string(),
            bytes_size_to_string(blobs.data_size),
            bytes_size_to_string(blobs.size),
        ]);
        total_count += blobs.count;
        total_data_size += blobs.data_size;
        total_size += blobs.size;
    }
    for blobs in &index_info.blobs_delete {
        if blobs.count > 0 {
            _ = table.add_row([
                format!("{:?} to delete", blobs.blob_type),
                blobs.count.to_string(),
                bytes_size_to_string(blobs.data_size),
                bytes_size_to_string(blobs.size),
            ]);
            total_count += blobs.count;
            total_data_size += blobs.data_size;
            total_size += blobs.size;
        }
    }

    _ = table.add_row([
        "Total".to_string(),
        total_count.to_string(),
        bytes_size_to_string(total_data_size),
        bytes_size_to_string(total_size),
    ]);

    println!();
    println!("{table}");

    let mut table = table_right_from(
        1,
        ["Blob type", "Pack Count", "Minimum Size", "Maximum Size"],
    );

    for packs in index_info.packs {
        _ = table.add_row([
            format!("{:?} packs", packs.blob_type),
            packs.count.to_string(),
            packs
                .min_size
                .map_or_else(|| "-".to_string(), bytes_size_to_string),
            packs
                .max_size
                .map_or_else(|| "-".to_string(), bytes_size_to_string),
        ]);
    }
    for packs in index_info.packs_delete {
        if packs.count > 0 {
            _ = table.add_row([
                format!("{:?} packs to delete", packs.blob_type),
                packs.count.to_string(),
                packs
                    .min_size
                    .map_or_else(|| "-".to_string(), bytes_size_to_string),
                packs
                    .max_size
                    .map_or_else(|| "-".to_string(), bytes_size_to_string),
            ]);
        }
    }
    println!();
    println!("{table}");
}
