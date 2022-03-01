use std::cell::RefCell;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::{anyhow, Result};

use crate::backend::DecryptWriteBackend;
use crate::blob::{BlobType, Metadata, Node, NodeType, Packer, Tree};
use crate::chunker::ChunkIter;
use crate::crypto::hash;
use crate::index::{IndexedBackend, Indexer};
use crate::repo::{SnapshotFile, TagList};

use super::Parent;

type SharedIndexer<BE> = Rc<RefCell<Indexer<BE>>>;

pub struct Archiver<BE: DecryptWriteBackend, I: IndexedBackend> {
    path: PathBuf,
    tree: Tree,
    parent: Parent<I>,
    stack: Vec<(Node, Tree, Parent<I>)>,
    size: u64,
    count: u64,
    be: BE,
    index: I,
    indexer: SharedIndexer<BE>,
    data_packer: Packer<BE>,
    tree_packer: Packer<BE>,
    poly: u64,
}

impl<BE: DecryptWriteBackend, I: IndexedBackend> Archiver<BE, I> {
    pub fn new(be: BE, index: I, poly: u64, parent: Parent<I>) -> Result<Self> {
        let indexer = Rc::new(RefCell::new(Indexer::new(be.clone())));
        Ok(Self {
            path: PathBuf::from("/"),
            tree: Tree::new(),
            parent: parent,
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

        self.finish_trees(&basepath)?;

        let missing_dirs = basepath.strip_prefix(&self.path)?;
        for p in missing_dirs.iter() {
            // new subdir
            self.path.push(p);
            let tree = std::mem::replace(&mut self.tree, Tree::new());
            if self.path == path {
                // use Node and return
                let new_parent = self.parent.sub_parent(&node);
                let parent = std::mem::replace(&mut self.parent, new_parent);
                self.stack.push((node, tree, parent));
                return Ok(());
            } else {
                let node = Node::new_dir(p.to_os_string(), Metadata::default());
                let new_parent = self.parent.sub_parent(&node);
                let parent = std::mem::replace(&mut self.parent, new_parent);
                self.stack.push((node, tree, parent));
            };
            println!("add tree {:?}, path: {:?}", p, self.path);
        }

        match node.node_type() {
            NodeType::File if r.is_some() => {
                println!("add file {:?}, path: {:?}", node.name(), self.path);
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
                self.tree_packer.add(&chunk, &id, BlobType::Tree)?;
            }

            let (mut node, tree, parent) = self.stack.pop().ok_or(anyhow!("tree stack empty??"))?;
            println!("finishing tree: {:?}", node.name());
            node.set_subtree(id);
            self.tree = tree;
            self.parent = parent;
            self.add_node(node, dirsize);
            self.path.pop();
        }
        Ok(())
    }

    pub fn backup_file(&mut self, node: Node, reader: impl BufRead) -> Result<()> {
        if let Some(p_node) = self.parent.is_parent(&node) {
            println!("using parent!");
            let size = *p_node.meta().size();
            let node = p_node.clone();
            self.add_node(node, size);
            return Ok(());
        }
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
        let mut node = node;
        node.set_content(content);
        self.add_node(node, filesize);
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
