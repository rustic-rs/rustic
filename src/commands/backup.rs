use std::cell::RefCell;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::rc::Rc;

use anyhow::Result;
use clap::Parser;
use ignore::WalkBuilder;

use crate::backend::{DecryptWriteBackend, ReadBackend};
use crate::blob::{BlobType, Node, Packer, Tree};
use crate::chunker::ChunkIter;
use crate::crypto::{hash, Key};
use crate::index::{AllIndexFiles, BoomIndex, Indexer, ReadIndex};
use crate::repo::{ConfigFile, SnapshotFile, TagList};

#[derive(Parser)]
pub(super) struct Opts {
    /// backup sources
    sources: Vec<String>,
}

pub(super) fn execute(
    opts: Opts,
    be: &(impl ReadBackend + DecryptWriteBackend),
    key: &Key,
) -> Result<()> {
    let config = ConfigFile::from_backend_no_id(be)?;

    let poly = u64::from_str_radix(config.chunker_polynomial(), 16)?;
    backup_file(opts.sources, &poly, be, key)?;
    Ok(())
}

fn backup_file(
    paths: Vec<String>,
    poly: &u64,
    be: &(impl ReadBackend + DecryptWriteBackend),
    key: &Key,
) -> Result<()> {
    let index: BoomIndex = AllIndexFiles::new(be.clone()).into_iter().collect();

    let indexer = Rc::new(RefCell::new(Indexer::new(be.clone())));
    let mut data_packer = Packer::new(be.clone(), indexer.clone(), key.clone())?;
    let mut tree_packer = Packer::new(be.clone(), indexer.clone(), key.clone())?;

    let path = &paths[0];
    let mut wb = WalkBuilder::new(path);
    /*
     for path in paths[1..].into_iter() {
        wb.add(path);
    }
    */

    wb.follow_links(false).hidden(false);

    let mut path = PathBuf::new();
    let mut tree = Tree::new();
    let mut names = Vec::new();
    let mut trees = Vec::new();
    let mut size: u64 = 0;
    let mut count: u64 = 0;

    for entry in wb.build() {
        let entry = entry?;
        // TODO
        let name = entry.file_name().to_string_lossy().to_string();
        let file_type = entry.file_type().unwrap();
        println!("{:?}, {:?}", entry.path(), path);

        if file_type.is_dir() {
            for p in entry.path().strip_prefix(&path).iter() {
                // new subdir
                trees.push(tree);
                tree = Tree::new();
                names.push(name.clone());
                path.push(p);
                println!("{:?}, {:?}", entry.path(), path);
            }
            continue;
        }

        while !entry.path().starts_with(&path) {
            // go back to parent dir
            // 1. finish tree
            let chunk = tree.serialize()?;
            let id = hash(&chunk);
            if !index.has(&id) {
                tree_packer.add(&chunk, &id, BlobType::Tree)?;
            }
            tree = trees.pop().unwrap();
            let name = names.pop().unwrap();
            let node = Node::from_tree(name, id);

            tree.add(node);
            path.pop();
            println!("{:?}, {:?}", entry.path(), path);
        }

        if file_type.is_file() {
            let f = File::open(&entry.path())?;
            let reader: BufReader<File> = BufReader::new(f);

            let chunk_iter = ChunkIter::new(reader, poly);
            let mut content = Vec::new();
            let mut filesize: u64 = 0;

            for chunk in chunk_iter {
                let chunk = chunk?;
                filesize += chunk.len() as u64;
                let id = hash(&chunk);
                if !index.has(&id) {
                    data_packer.add(&chunk, &id, BlobType::Data)?;
                }
                content.push(id);
            }
            let node = Node::from_content(name, content, filesize);
            tree.add(node);
            count += 1;
            size += filesize;
        }
    }

    loop {
        // go back to parent dir
        // 1. finish tree
        let chunk = tree.serialize()?;
        let id = hash(&chunk);
        if !index.has(&id) {
            tree_packer.add(&chunk, &id, BlobType::Tree)?;
        }
        tree = match trees.pop() {
            Some(tree) => tree,
            None => break,
        };
        let name = names.pop().unwrap();
        let node = Node::from_tree(name, id);

        tree.add(node);
        path.pop();
    }

    let chunk = tree.serialize()?;
    let id = hash(&chunk);
    if !index.has(&id) {
        tree_packer.add(&chunk, &id, BlobType::Tree)?;
    }

    data_packer.finalize()?;
    tree_packer.finalize()?;
    indexer.borrow().finalize()?;

    // save snapshot
    let snap = SnapshotFile::new(
        id,
        paths,
        "host".to_string(),
        "user".to_string(),
        0,
        0,
        TagList::default(),
        Some(count),
        Some(size),
    );
    let id = snap.save_to_backend(be)?;
    println!("snapshot {} successfully saved.", id);

    Ok(())
}
