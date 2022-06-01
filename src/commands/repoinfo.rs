use anyhow::Result;
use clap::Parser;
use futures::TryStreamExt;
use prettytable::{cell, format, row, Table};
use vlog::*;

use super::{bytes, progress_counter};
use crate::backend::{DecryptReadBackend, ALL_FILE_TYPES};
use crate::blob::BlobType;
use crate::index::IndexEntry;
use crate::repo::IndexFile;

#[derive(Parser)]
pub(super) struct Opts;

pub(super) async fn execute(be: &impl DecryptReadBackend, _opts: Opts) -> Result<()> {
    v1!("scanning files...");

    let mut table = Table::new();
    let mut total_count = 0;
    let mut total_size = 0;
    for tpe in ALL_FILE_TYPES {
        let list = be.list_with_size(tpe).await?;
        let count = list.len();
        let size = list.iter().map(|f| f.1 as u64).sum();
        table.add_row(row![format!("{:?}", tpe), r->count, r->bytes(size)]);
        total_count += count;
        total_size += size;
    }
    table.add_row(row!["Total",r->total_count,r->bytes(total_size)]);

    table.set_titles(row![b->"File type", br->"Count", br->"Total Size"]);
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
    println!();
    table.printstd();
    println!();

    v1!("scanning index...");
    let p = progress_counter();
    let mut stream = be.stream_all::<IndexFile>(p.clone()).await?;

    #[derive(Default)]
    struct Info {
        count: u64,
        size: u64,
        data_size: u64,
    }

    impl Info {
        fn add(&mut self, ie: IndexEntry) {
            self.count += 1;
            self.size += *ie.length() as u64;
            self.data_size += ie.data_length() as u64;
        }
    }

    let mut tree = Info::default();
    let mut data = Info::default();
    let mut tree_delete = Info::default();
    let mut data_delete = Info::default();

    while let Some((_, index)) = stream.try_next().await? {
        for pack in &index.packs {
            for blob in &pack.blobs {
                let ie = IndexEntry::from_index_blob(blob, pack.id);
                match blob.tpe {
                    BlobType::Tree => tree.add(ie),
                    BlobType::Data => data.add(ie),
                }
            }
        }

        for pack in &index.packs_to_delete {
            for blob in &pack.blobs {
                let ie = IndexEntry::from_index_blob(blob, pack.id);
                match blob.tpe {
                    BlobType::Tree => tree_delete.add(ie),
                    BlobType::Data => data_delete.add(ie),
                }
            }
        }
    }
    p.finish_with_message("done");

    let mut table = Table::new();

    table.add_row(row!["Tree",r->tree.count,r->bytes(tree.data_size), r->bytes(tree.size)]);
    table.add_row(row!["Data",r->data.count,r->bytes(data.data_size),r->bytes(data.size)]);
    if tree_delete.count > 0 {
        table.add_row(row!["Tree to delete",r->tree_delete.count,r->bytes(tree_delete.data_size),r->bytes(tree_delete.size)]);
    }
    if data_delete.count > 0 {
        table.add_row(row!["Data to delete",r->data_delete.count,r->bytes(data_delete.data_size),r->bytes(data_delete.size)]);
    }
    table.add_row(
        row!["Total",r->tree.count + data.count+tree_delete.count + data_delete.count,
        r->bytes(tree.data_size+data.data_size+tree_delete.data_size+data_delete.data_size),
        r->bytes(tree.size+data.size+tree_delete.size+data_delete.size)],
    );

    table.set_titles(row![b->"Blob type", br->"Count", br->"Total Size",br->"Total Size in Packs"]);
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
    println!();
    table.printstd();

    Ok(())
}
