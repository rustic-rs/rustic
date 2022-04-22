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
    let mut tree_count = 0;
    let mut tree_size = 0;
    let mut data_count = 0;
    let mut data_size = 0;
    while let Some(index) = stream.next().await {
        for pack in index?.1.packs {
            for blob in pack.blobs {
                match blob.tpe {
                    BlobType::Tree => {
                        tree_count += 1;
                        tree_size += blob.length as u64;
                    }
                    BlobType::Data => {
                        data_count += 1;
                        data_size += blob.length as u64;
                    }
                }
            }
        }
    }
    p.finish_with_message("done");

    let mut table = Table::new();
    table.add_row(row!["Tree",r->tree_count,r->ByteSize(tree_size).to_string_as(true)]);
    table.add_row(row!["Data",r->data_count,r->ByteSize(data_size).to_string_as(true)]);
    table.add_row(row!["Total",r->tree_count + data_count,r->ByteSize(tree_size+data_size).to_string_as(true)]);

    table.set_titles(row![b->"Blob type", br->"Count", br->"Total Size"]);
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
    println!();
    table.printstd();

    Ok(())
}
