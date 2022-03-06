use std::cell::RefCell;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::{anyhow, Result};
use bytesize::ByteSize;

use crate::backend::DecryptWriteBackend;
use crate::blob::{BlobType, Metadata, Node, NodeType, Packer, Tree};
use crate::chunker::ChunkIter;
use crate::crypto::hash;
use crate::index::{IndexedBackend, Indexer};
use crate::repo::SnapshotFile;

use super::{Parent, ParentResult};

type SharedIndexer<BE> = Rc<RefCell<Indexer<BE>>>;

pub struct Archiver<BE: DecryptWriteBackend, I: IndexedBackend> {
    path: PathBuf,
    tree: Tree,
    parent: Parent<I>,
    stack: Vec<(Node, Tree, Parent<I>)>,
    index: I,
    indexer: SharedIndexer<BE>,
    data_packer: Packer<BE>,
    tree_packer: Packer<BE>,
    be: BE,
    poly: u64,
    size: u64,
    count: u64,
    files_new: u64,
    files_changed: u64,
    files_unmodified: u64,
    data_blobs_written: u64,
    tree_blobs_written: u64,
    data_added: u64,
}

impl<BE: DecryptWriteBackend, I: IndexedBackend> Archiver<BE, I> {
    pub fn new(be: BE, index: I, poly: u64, parent: Parent<I>) -> Result<Self> {
        let indexer = Rc::new(RefCell::new(Indexer::new(be.clone())));
        Ok(Self {
            path: PathBuf::from("/"),
            tree: Tree::new(),
            parent,
            stack: Vec::new(),
            index,
            data_packer: Packer::new(be.clone(), indexer.clone())?,
            tree_packer: Packer::new(be.clone(), indexer.clone())?,
            be,
            poly,
            indexer,
            size: 0,
            count: 0,
            files_new: 0,
            files_changed: 0,
            files_unmodified: 0,
            data_blobs_written: 0,
            tree_blobs_written: 0,
            data_added: 0,
        })
    }

    pub fn add_node(&mut self, node: Node, size: u64) {
        self.tree.add(node);
        self.count += 1;
        self.size += size;
    }

    pub fn add_entry(&mut self, path: &Path, node: Node, r: Option<impl BufRead>) -> Result<()> {
        let basepath = if node.is_dir() {
            path
        } else {
            path.parent()
                .ok_or(anyhow!("file path should have a parent!"))?
        };

        self.finish_trees(basepath)?;

        let missing_dirs = basepath.strip_prefix(&self.path)?;
        for p in missing_dirs.iter() {
            // new subdir
            self.path.push(p);
            let tree = std::mem::replace(&mut self.tree, Tree::new());
            if self.path == path {
                // use Node and return
                let new_parent = self.parent.sub_parent(&node)?;
                let parent = std::mem::replace(&mut self.parent, new_parent);
                self.stack.push((node, tree, parent));
                return Ok(());
            } else {
                let node = Node::new_dir(p.to_os_string(), Metadata::default());
                let new_parent = self.parent.sub_parent(&node)?;
                let parent = std::mem::replace(&mut self.parent, new_parent);
                self.stack.push((node, tree, parent));
            };
        }

        match node.node_type() {
            NodeType::File if r.is_some() => {
                self.backup_file(node, r.unwrap())?;
            }
            NodeType::Dir => {}          // is already handled, see above
            _ => self.add_node(node, 0), // all other cases: just save the given node
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
                if self.tree_packer.add(&chunk, &id, BlobType::Tree)? {
                    self.tree_blobs_written += 1;
                    self.data_added += dirsize;
                    println!(
                        "new       tree: {:?} {}",
                        self.path,
                        ByteSize(dirsize).to_string_as(true)
                    );
                } else {
                    println!("unchanged tree: {:?}", self.path);
                }
            }

            let (mut node, tree, parent) = self.stack.pop().ok_or(anyhow!("tree stack empty??"))?;
            node.set_subtree(id);
            self.tree = tree;
            self.parent = parent;
            self.add_node(node, dirsize);
            self.path.pop();
        }
        Ok(())
    }

    pub fn backup_file(&mut self, node: Node, reader: impl BufRead) -> Result<()> {
        match self.parent.is_parent(&node) {
            ParentResult::Matched(p_node) => {
                println!("unchanged file: {:?} {}", self.path, node.name());
                self.files_unmodified += 1;
                if p_node.content().iter().all(|id| self.index.has(id)) {
                    let size = *p_node.meta().size();
                    let mut node = node;
                    node.set_content(p_node.content().to_vec());
                    self.add_node(node, size);
                    return Ok(());
                } else {
                    eprintln!("missing blobs for node in index!");
                }
            }
            ParentResult::NotMatched => {
                println!("changed   file: {:?} {}", self.path, node.name());
                self.files_changed += 1;
            }
            ParentResult::NotFound => {
                println!("new       file: {:?} {}", self.path, node.name());
                self.files_new += 1;
            }
        }
        let chunk_iter = ChunkIter::new(reader, &self.poly);
        let mut content = Vec::new();
        let mut filesize: u64 = 0;

        for chunk in chunk_iter {
            let chunk = chunk?;
            filesize += chunk.len() as u64;
            let id = hash(&chunk);
            if !self.index.has(&id) && self.data_packer.add(&chunk, &id, BlobType::Data)? {
                self.data_blobs_written += 1;
                self.data_added += filesize;
            }
            content.push(id);
        }
        let mut node = node;
        node.set_content(content);
        self.add_node(node, filesize);
        Ok(())
    }

    pub fn finalize_snapshot(&mut self, mut snap: SnapshotFile) -> Result<()> {
        self.finish_trees(&PathBuf::from("/"))?;

        let chunk = self.tree.serialize()?;
        let id = hash(&chunk);
        if !self.index.has(&id) {
            self.tree_packer.add(&chunk, &id, BlobType::Tree)?;
        }

        self.data_packer.finalize()?;
        self.tree_packer.finalize()?;
        self.indexer.borrow().finalize()?;

        snap.set_tree(id);
        snap.set_size(self.size);
        snap.set_count(self.count);

        let id = snap.save_to_backend(&self.be)?;

        println!(
            "Files:       {} new, {} changed, {} unmodified",
            self.files_new, self.files_changed, self.files_unmodified
        );
        println!("Data Blobs:  {} new", self.data_blobs_written);
        println!("Tree Blobs:  {} new", self.tree_blobs_written);
        println!(
            "Added to the repo: {}",
            ByteSize(self.data_added).to_string_as(true)
        );
        println!(
            "processed {} nodes, {}",
            self.count,
            ByteSize(self.size).to_string_as(true)
        );
        println!("snapshot {} successfully saved.", id);
        Ok(())
    }
}
