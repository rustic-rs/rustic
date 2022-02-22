use std::cell::RefCell;
use std::ffi::OsString;
use std::fs::{File, FileType};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::{anyhow, Result};

use crate::backend::DecryptWriteBackend;
use crate::blob::{BlobType, Node, Packer, Tree};
use crate::chunker::ChunkIter;
use crate::crypto::hash;
use crate::index::{Indexer, ReadIndex};
use crate::repo::{SnapshotFile, TagList};

pub type SharedIndexer<BE> = Rc<RefCell<Indexer<BE>>>;

pub struct Archiver<BE: DecryptWriteBackend, I: ReadIndex> {
    path: PathBuf,
    tree: Tree,
    stack: Vec<(Node, Tree)>,
    size: u64,
    count: u64,
    be: BE,
    index: I,
    indexer: SharedIndexer<BE>,
    data_packer: Packer<BE>,
    tree_packer: Packer<BE>,
    poly: u64,
}

impl<BE: DecryptWriteBackend, I: ReadIndex> Archiver<BE, I> {
    pub fn new(be: BE, index: I, poly: u64) -> Result<Self> {
        let indexer = Rc::new(RefCell::new(Indexer::new(be.clone())));
        Ok(Self {
            path: PathBuf::from("/"),
            tree: Tree::new(),
            stack: Vec::new(),
            size: 0,
            count: 0,
            index,
            data_packer: Packer::new(be.clone(), indexer.clone())?,
            tree_packer: Packer::new(be.clone(), indexer.clone())?,
            poly,
            be,
            indexer,
        })
    }

    pub fn add_entry(&mut self, path: &Path, name: OsString, file_type: FileType) -> Result<()> {
        let basepath = if file_type.is_dir() {
            path
        } else {
            path.parent()
                .ok_or(anyhow!("file path should have a parent!"))?
        };

        self.finish_trees(&basepath)?;

        let missing_dirs = basepath.strip_prefix(&self.path)?;

        for p in missing_dirs.iter() {
            // new subdir
            let tree = std::mem::replace(&mut self.tree, Tree::new());
            let node = Node::new_tree(p.to_os_string())?;
            self.stack.push((node, tree));
            self.path.push(p);
            println!("add tree {:?}, path: {:?}", p, self.path);
        }

        if file_type.is_file() {
            println!("add file {:?}, path: {:?}", name, self.path);
            let f = File::open(&path)?;
            let reader: BufReader<File> = BufReader::new(f);
            self.backup_file(name, reader)?;
        }
        Ok(())
    }

    pub fn finish_trees(&mut self, path: &Path) -> Result<()> {
        while !path.starts_with(&self.path) {
            // go back to parent dir
            let chunk = self.tree.serialize()?;
            let id = hash(&chunk);
            let dirsize: u64 = chunk.len().try_into()?;
            if !self.index.has(&id) {
                self.tree_packer.add(&chunk, &id, BlobType::Tree)?;
            }

            let (mut node, tree) = self.stack.pop().ok_or(anyhow!("tree stack empty??"))?;
            println!("finishing tree: {:?}", node.name());
            node.set_subtree(id);
            self.tree = tree;
            self.tree.add(node);
            self.count += 1;
            self.size += dirsize;
            self.path.pop();
        }
        Ok(())
    }

    pub fn backup_file(&mut self, name: OsString, reader: impl BufRead) -> Result<()> {
        let chunk_iter = ChunkIter::new(reader, &self.poly);
        let mut content = Vec::new();
        let mut filesize: u64 = 0;

        for chunk in chunk_iter {
            let chunk = chunk?;
            filesize += chunk.len() as u64;
            let id = hash(&chunk);
            if !self.index.has(&id) {
                self.data_packer.add(&chunk, &id, BlobType::Data)?;
            }
            content.push(id);
        }
        let node = Node::from_content(name, content, filesize)?;
        self.tree.add(node);
        self.count += 1;
        self.size += filesize;
        Ok(())
    }

    pub fn finalize_snapshot(&mut self, backup_path: PathBuf) -> Result<()> {
        self.finish_trees(&PathBuf::from("/"))?;

        let chunk = self.tree.serialize()?;
        let id = hash(&chunk);
        if !self.index.has(&id) {
            self.tree_packer.add(&chunk, &id, BlobType::Tree)?;
        }

        self.data_packer.finalize()?;
        self.tree_packer.finalize()?;
        self.indexer.borrow().finalize()?;

        // save snapshot
        let snap = SnapshotFile::new(
            id,
            vec![backup_path
                .to_str()
                .ok_or(anyhow!("non-unicode path {:?}", backup_path))?
                .to_string()],
            "host".to_string(),
            "user".to_string(),
            0,
            0,
            TagList::default(),
            Some(self.count),
            Some(self.size),
        );
        let id = snap.save_to_backend(&self.be)?;
        println!("snapshot {} successfully saved.", id);
        Ok(())
    }
}
