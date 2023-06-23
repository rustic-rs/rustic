//! `repoinfo` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{
    commands::get_repository, helpers::bytes_size_to_string, status_err, Application, RUSTIC_APP,
};

use abscissa_core::{Command, Runnable, Shutdown};

use crate::helpers::table_right_from;
use anyhow::Result;
use rustic_core::RepoFileInfo;

/// `repoinfo` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct RepoInfoCmd;

impl Runnable for RepoInfoCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl RepoInfoCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();
        let repo = get_repository(&config);
        let file_info = repo.infos_files()?;
        print_file_info("repository files", file_info.files);
        if let Some(info) = file_info.files_hot {
            print_file_info("hot repository files", info);
        }

        let repo = repo.open()?;
        let index_info = repo.infos_index()?;

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
                    .map_or("-".to_string(), |s| bytes_size_to_string(s)),
                packs
                    .max_size
                    .map_or("-".to_string(), |s| bytes_size_to_string(s)),
            ]);
        }
        for packs in index_info.packs_delete {
            if packs.count > 0 {
                _ = table.add_row([
                    format!("{:?} packs to delete", packs.blob_type),
                    packs.count.to_string(),
                    packs
                        .min_size
                        .map_or("-".to_string(), |s| bytes_size_to_string(s)),
                    packs
                        .max_size
                        .map_or("-".to_string(), |s| bytes_size_to_string(s)),
                ]);
            }
        }
        println!();
        println!("{table}");

        Ok(())
    }
}

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
            packs.min_size.map_or("-".to_string(), bytes_size_to_string),
            packs.max_size.map_or("-".to_string(), bytes_size_to_string),
        ]);
    }
    for packs in index_info.packs_delete {
        if packs.count > 0 {
            _ = table.add_row([
                format!("{:?} packs to delete", packs.blob_type),
                packs.count.to_string(),
                packs.min_size.map_or("-".to_string(), bytes_size_to_string),
                packs.max_size.map_or("-".to_string(), bytes_size_to_string),
            ]);
        }
    }
    println!();
    println!("{table}");
}
