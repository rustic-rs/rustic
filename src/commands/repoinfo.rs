use anyhow::Result;
use bytesize::ByteSize;
use clap::Parser;
use futures::StreamExt;
use prettytable::{cell, format, row, Table};
use vlog::*;

use super::progress_counter;
use crate::backend::{DecryptReadBackend, ALL_FILE_TYPES};
use crate::blob::BlobType;
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
        table.add_row(row![
            format!("{:?}", tpe),
            r->count,
            r->ByteSize(size).to_string_as(true)
        ]);
        total_count += count;
        total_size += size;
    }
    table.add_row(row!["Total",r->total_count,r->ByteSize(total_size).to_string_as(true)]);

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
    }

    impl Info {
        fn add(&mut self, length: u32) {
            self.count += 1;
            self.size += length as u64;
        }
    }

    let mut tree = Info::default();
    let mut data = Info::default();
    let mut tree_delete = Info::default();
    let mut data_delete = Info::default();

    while let Some(index) = stream.next().await {
        let index = index?.1;
        for blob in index.packs.iter().flat_map(|pack| &pack.blobs) {
            match blob.tpe {
                BlobType::Tree => tree.add(blob.length),
                BlobType::Data => data.add(blob.length),
            }
        }
        for blob in index.packs_to_delete.iter().flat_map(|pack| &pack.blobs) {
            match blob.tpe {
                BlobType::Tree => tree_delete.add(blob.length),
                BlobType::Data => data_delete.add(blob.length),
            }
        }
    }
    p.finish_with_message("done");

    let mut table = Table::new();
    table.add_row(row!["Tree",r->tree.count,r->ByteSize(tree.size).to_string_as(true)]);
    table.add_row(row!["Data",r->data.count,r->ByteSize(data.size).to_string_as(true)]);
    if tree_delete.count > 0 {
        table.add_row(
            row!["Tree to delete",r->tree_delete.count,r->ByteSize(tree_delete.size).to_string_as(true)],
        );
    }
    if data_delete.count > 0 {
        table.add_row(
            row!["Data to delete",r->data_delete.count,r->ByteSize(data_delete.size).to_string_as(true)],
        );
    }
    table.add_row(
        row!["Total",r->tree.count + data.count+tree_delete.count + data_delete.count,
        r->ByteSize(tree.size+data.size+tree_delete.size+data_delete.size).to_string_as(true)],
    );

    table.set_titles(row![b->"Blob type", br->"Count", br->"Total Size"]);
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
    println!();
    table.printstd();

    Ok(())
}
