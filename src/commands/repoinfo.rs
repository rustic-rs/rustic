use anyhow::Result;
use clap::Parser;
use derive_more::Add;
use log::*;

use super::{bytes, progress_counter, table_right_from};
use crate::backend::{DecryptReadBackend, ReadBackend, ALL_FILE_TYPES};
use crate::blob::{BlobType, BlobTypeMap, Sum};
use crate::index::IndexEntry;
use crate::repofile::{IndexFile, IndexPack};
use crate::repository::OpenRepository;

#[derive(Parser)]
pub(super) struct Opts;

pub(super) fn execute(repo: OpenRepository, _opts: Opts) -> Result<()> {
    fileinfo("repository files", &repo.be)?;
    if let Some(hot_be) = &repo.be_hot {
        fileinfo("hot repository files", hot_be)?;
    }

    #[derive(Default, Clone, Copy, Add)]
    struct Info {
        count: u64,
        size: u64,
        data_size: u64,
        pack_count: u64,
        total_pack_size: u64,
        min_pack_size: u64,
        max_pack_size: u64,
    }

    impl Info {
        fn add(&mut self, ie: IndexEntry) {
            self.count += 1;
            self.size += *ie.length() as u64;
            self.data_size += ie.data_length() as u64;
        }

        fn add_pack(&mut self, ip: &IndexPack) {
            self.pack_count += 1;
            let size = ip.pack_size() as u64;
            self.total_pack_size += size;
            self.min_pack_size = self.min_pack_size.min(size);
            self.max_pack_size = self.max_pack_size.max(size);
        }
    }

    let mut info = BlobTypeMap::<Info>::default();
    info[BlobType::Tree].min_pack_size = u64::MAX;
    info[BlobType::Data].min_pack_size = u64::MAX;
    let mut info_delete = BlobTypeMap::<Info>::default();

    let p = progress_counter("scanning index...");
    for index in repo.dbe.stream_all::<IndexFile>(p.clone())? {
        let index = index?.1;
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
    }
    p.finish_with_message("done");

    let mut table = table_right_from(
        1,
        ["Blob type", "Count", "Total Size", "Total Size in Packs"],
    );

    for (blob_type, info) in &info {
        table.add_row([
            format!("{blob_type:?}"),
            info.count.to_string(),
            bytes(info.data_size),
            bytes(info.size),
        ]);
    }

    for (blob_type, info_delete) in &info_delete {
        if info_delete.count > 0 {
            table.add_row([
                format!("{blob_type:?} to delete"),
                info_delete.count.to_string(),
                bytes(info_delete.data_size),
                bytes(info_delete.size),
            ]);
        }
    }
    let total = info.sum() + info_delete.sum();
    table.add_row([
        "Total".to_string(),
        total.count.to_string(),
        bytes(total.data_size),
        bytes(total.size),
    ]);

    println!();
    println!("{table}");

    let mut table = table_right_from(
        1,
        ["Blob type", "Pack Count", "Minimum Size", "Maximum Size"],
    );

    for (blob_type, info) in info {
        table.add_row([
            format!("{blob_type:?} packs"),
            info.pack_count.to_string(),
            bytes(info.min_pack_size),
            bytes(info.max_pack_size),
        ]);
    }
    println!();
    println!("{table}");

    Ok(())
}

fn fileinfo(text: &str, be: &impl ReadBackend) -> Result<()> {
    info!("scanning files...");

    let mut table = table_right_from(1, ["File type", "Count", "Total Size"]);
    let mut total_count = 0;
    let mut total_size = 0;
    for tpe in ALL_FILE_TYPES {
        let list = be.list_with_size(tpe)?;
        let count = list.len();
        let size = list.iter().map(|f| f.1 as u64).sum();
        table.add_row([format!("{:?}", tpe), count.to_string(), bytes(size)]);
        total_count += count;
        total_size += size;
    }
    println!("{}", text);
    table.add_row([
        "Total".to_string(),
        total_count.to_string(),
        bytes(total_size),
    ]);

    println!();
    println!("{table}");
    println!();
    Ok(())
}
