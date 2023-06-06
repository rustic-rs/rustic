//! `repoinfo` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{
    commands::{get_repository, open_repository},
    status_err, Application, RUSTIC_APP,
};

use abscissa_core::{Command, Runnable, Shutdown};

use anyhow::Result;
use rustic_core::helpers::table_output::{print_file_info, table_right_from};
use rustic_core::{
    bytes_size_to_string, BlobType, BlobTypeMap, DecryptReadBackend, IndexEntry, IndexFile,
    RepoInfo, Sum,
};

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
        let repo = open_repository(get_repository(&config));

        print_file_info("repository files", &repo.be)?;

        if let Some(hot_be) = &repo.be_hot {
            print_file_info("hot repository files", hot_be)?;
        }

        let mut info = BlobTypeMap::<RepoInfo>::default();
        info[BlobType::Tree].min_pack_size = u64::MAX;
        info[BlobType::Data].min_pack_size = u64::MAX;
        let mut info_delete = BlobTypeMap::<RepoInfo>::default();

        let p = config
            .global
            .progress_options
            .progress_counter("scanning index...");
        repo.dbe
            .stream_all::<IndexFile>(p.clone())?
            .into_iter()
            .for_each(|index| {
                let index = match index {
                    Ok(it) => it,
                    Err(err) => {
                        status_err!("{}", err);
                        RUSTIC_APP.shutdown(Shutdown::Crash);
                    }
                }
                .1;
                for pack in &index.packs {
                    info[pack.blob_type()].add_pack(pack);

                    for blob in &pack.blobs {
                        let ie = IndexEntry::from_index_blob(blob, pack.id);
                        info[pack.blob_type()].add(ie);
                    }
                }

                for pack in &index.packs_to_delete {
                    for blob in &pack.blobs {
                        let ie = IndexEntry::from_index_blob(blob, pack.id);
                        info_delete[pack.blob_type()].add(ie);
                    }
                }
            });
        p.finish_with_message("done");

        let mut table = table_right_from(
            1,
            ["Blob type", "Count", "Total Size", "Total Size in Packs"],
        );

        for (blob_type, info) in &info {
            _ = table.add_row([
                format!("{blob_type:?}"),
                info.count.to_string(),
                bytes_size_to_string(info.data_size),
                bytes_size_to_string(info.size),
            ]);
        }

        for (blob_type, info_delete) in &info_delete {
            if info_delete.count > 0 {
                _ = table.add_row([
                    format!("{blob_type:?} to delete"),
                    info_delete.count.to_string(),
                    bytes_size_to_string(info_delete.data_size),
                    bytes_size_to_string(info_delete.size),
                ]);
            }
        }
        let total = info.sum() + info_delete.sum();
        _ = table.add_row([
            "Total".to_string(),
            total.count.to_string(),
            bytes_size_to_string(total.data_size),
            bytes_size_to_string(total.size),
        ]);

        println!();
        println!("{table}");

        let mut table = table_right_from(
            1,
            ["Blob type", "Pack Count", "Minimum Size", "Maximum Size"],
        );

        for (blob_type, info) in info {
            _ = table.add_row([
                format!("{blob_type:?} packs"),
                info.pack_count.to_string(),
                bytes_size_to_string(info.min_pack_size),
                bytes_size_to_string(info.max_pack_size),
            ]);
        }
        println!();
        println!("{table}");

        Ok(())
    }
}
