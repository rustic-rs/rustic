use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};
use std::ffi::{OsStr, OsString};
use std::mem;
use std::path::{Component, Path, PathBuf, Prefix};
use std::str;

use anyhow::{anyhow, bail, Result};
use clap::Parser;
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use ignore::overrides::{Override, OverrideBuilder};
use ignore::Match;
use indicatif::ProgressBar;
use serde::{Deserialize, Deserializer, Serialize};

use crate::crypto::hash;
use crate::id::Id;
use crate::index::IndexedBackend;
use crate::repofile::SnapshotSummary;

use super::{Metadata, Node, NodeType};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tree {
    #[serde(deserialize_with = "deserialize_null_default")]
    pub nodes: Vec<Node>,
}

fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    T: Default + Deserialize<'de>,
    D: Deserializer<'de>,
{
    let opt = Option::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

impl Tree {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    pub fn add(&mut self, node: Node) {
        self.nodes.push(node);
    }

    pub fn serialize(&self) -> Result<(Vec<u8>, Id)> {
        let mut chunk = serde_json::to_vec(&self)?;
        chunk.push(b'\n'); // for whatever reason, restic adds a newline, so to be compatible...
        let id = hash(&chunk);
        Ok((chunk, id))
    }

    pub fn from_backend(be: &impl IndexedBackend, id: Id) -> Result<Self> {
        let data = be
            .get_tree(&id)
            .ok_or_else(|| anyhow!("blob {id:?} not found in index"))?
            .read_data(be.be())?;

        Ok(serde_json::from_slice(&data)?)
    }

    pub fn node_from_path(be: &impl IndexedBackend, id: Id, path: &Path) -> Result<Node> {
        let mut node = Node::new_node(OsStr::new(""), NodeType::Dir, Metadata::default());
        node.subtree = Some(id);

        for p in path.components() {
            if let Some(p) = comp_to_osstr(p)? {
                let id = node.subtree.ok_or_else(|| anyhow!("{p:?} is no dir"))?;
                let tree = Tree::from_backend(be, id)?;
                node = tree
                    .nodes
                    .into_iter()
                    .find(|node| node.name() == p)
                    .ok_or_else(|| anyhow!("{p:?} not found"))?;
            }
        }

        Ok(node)
    }
}

pub fn comp_to_osstr(p: Component) -> Result<Option<OsString>> {
    let s = match p {
        Component::RootDir => None,
        Component::Prefix(p) => match p.kind() {
            Prefix::Verbatim(p) | Prefix::DeviceNS(p) => Some(p.to_os_string()),
            Prefix::VerbatimUNC(_, q) | Prefix::UNC(_, q) => Some(q.to_os_string()),
            Prefix::VerbatimDisk(p) | Prefix::Disk(p) => {
                Some(OsStr::new(str::from_utf8(&[p])?).to_os_string())
            }
        },
        Component::Normal(p) => Some(p.to_os_string()),
        _ => bail!("path should not contain current or parent dir"),
    };
    Ok(s)
}

impl IntoIterator for Tree {
    type Item = Node;
    type IntoIter = std::vec::IntoIter<Node>;

    fn into_iter(self) -> Self::IntoIter {
        self.nodes.into_iter()
    }
}

#[derive(Default, Clone, Parser)]
pub struct TreeStreamerOptions {
    /// Glob pattern to exclude/include (can be specified multiple times)
    #[clap(long, help_heading = "Exclude options")]
    glob: Vec<String>,

    /// Same as --glob pattern but ignores the casing of filenames
    #[clap(long, value_name = "GLOB", help_heading = "Exclude options")]
    iglob: Vec<String>,

    /// Read glob patterns to exclude/include from this file (can be specified multiple times)
    #[clap(long, value_name = "FILE", help_heading = "Exclude options")]
    glob_file: Vec<String>,

    /// Same as --glob-file ignores the casing of filenames in patterns
    #[clap(long, value_name = "FILE", help_heading = "Exclude options")]
    iglob_file: Vec<String>,
}

/// [`NodeStreamer`] recursively streams all nodes of a given tree including all subtrees in-order
pub struct NodeStreamer<BE>
where
    BE: IndexedBackend,
{
    open_iterators: Vec<std::vec::IntoIter<Node>>,
    inner: std::vec::IntoIter<Node>,
    path: PathBuf,
    be: BE,
    overrides: Option<Override>,
}

impl<BE> NodeStreamer<BE>
where
    BE: IndexedBackend,
{
    pub fn new(be: BE, node: &Node) -> Result<Self> {
        Self::new_streamer(be, node, None)
    }

    fn new_streamer(be: BE, node: &Node, overrides: Option<Override>) -> Result<Self> {
        let inner = if node.is_dir() {
            Tree::from_backend(&be, node.subtree.unwrap())?
                .nodes
                .into_iter()
        } else {
            vec![node.clone()].into_iter()
        };
        Ok(Self {
            inner,
            open_iterators: Vec::new(),
            path: PathBuf::new(),
            be,
            overrides,
        })
    }

    pub fn new_with_glob(be: BE, node: &Node, opts: TreeStreamerOptions) -> Result<Self> {
        let mut override_builder = OverrideBuilder::new("/");

        for g in opts.glob {
            override_builder.add(&g)?;
        }

        for file in opts.glob_file {
            for line in std::fs::read_to_string(file)?.lines() {
                override_builder.add(line)?;
            }
        }

        override_builder.case_insensitive(true)?;
        for g in opts.iglob {
            override_builder.add(&g)?;
        }

        for file in opts.iglob_file {
            for line in std::fs::read_to_string(file)?.lines() {
                override_builder.add(line)?;
            }
        }
        let overrides = override_builder.build()?;

        Self::new_streamer(be, node, Some(overrides))
    }
}

type NodeStreamItem = Result<(PathBuf, Node)>;

// TODO: This is not parallel at the moment...
impl<BE> Iterator for NodeStreamer<BE>
where
    BE: IndexedBackend,
{
    type Item = NodeStreamItem;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                Some(node) => {
                    let path = self.path.join(node.name());
                    if let Some(id) = node.subtree {
                        self.path.push(node.name());
                        let be = self.be.clone();
                        let tree = match Tree::from_backend(&be, id) {
                            Ok(tree) => tree,
                            Err(err) => return Some(Err(err)),
                        };
                        let old_inner = mem::replace(&mut self.inner, tree.nodes.into_iter());
                        self.open_iterators.push(old_inner);
                    }

                    if let Some(overrides) = &self.overrides {
                        if let Match::Ignore(_) = overrides.matched(&path, false) {
                            continue;
                        }
                    }

                    return Some(Ok((path, node)));
                }
                None => match self.open_iterators.pop() {
                    Some(it) => {
                        self.inner = it;
                        self.path.pop();
                    }
                    None => return None,
                },
            }
        }
    }
}

/// [`TreeStreamerOnce`] recursively visits all trees and subtrees, but each tree ID only once
pub struct TreeStreamerOnce {
    visited: HashSet<Id>,
    queue_in: Option<Sender<(PathBuf, Id, usize)>>,
    queue_out: Receiver<Result<(PathBuf, Tree, usize)>>,
    p: ProgressBar,
    counter: Vec<usize>,
    finished_ids: usize,
}

const MAX_TREE_LOADER: usize = 4;

impl TreeStreamerOnce {
    pub fn new<BE: IndexedBackend>(be: BE, ids: Vec<Id>, p: ProgressBar) -> Result<Self> {
        p.set_length(ids.len() as u64);

        let (out_tx, out_rx) = bounded(MAX_TREE_LOADER);
        let (in_tx, in_rx) = unbounded();

        for _ in 0..MAX_TREE_LOADER {
            let be = be.clone();
            let in_rx = in_rx.clone();
            let out_tx = out_tx.clone();
            std::thread::spawn(move || {
                for (path, id, count) in in_rx {
                    out_tx
                        .send(Tree::from_backend(&be, id).map(|tree| (path, tree, count)))
                        .unwrap();
                }
            });
        }

        let counter = vec![0; ids.len()];
        let mut streamer = Self {
            visited: HashSet::new(),
            queue_in: Some(in_tx),
            queue_out: out_rx,
            p,
            counter,
            finished_ids: 0,
        };

        for (count, id) in ids.into_iter().enumerate() {
            if !streamer.add_pending(PathBuf::new(), id, count)? {
                streamer.p.inc(1);
                streamer.finished_ids += 1;
            }
        }

        Ok(streamer)
    }

    fn add_pending(&mut self, path: PathBuf, id: Id, count: usize) -> Result<bool> {
        if self.visited.insert(id) {
            self.queue_in.as_ref().unwrap().send((path, id, count))?;
            self.counter[count] += 1;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

type TreeStreamItem = Result<(PathBuf, Tree)>;

impl Iterator for TreeStreamerOnce {
    type Item = TreeStreamItem;

    fn next(&mut self) -> Option<Self::Item> {
        if self.counter.len() == self.finished_ids {
            drop(self.queue_in.take());
            self.p.finish();
            return None;
        }
        let (path, tree, count) = match self.queue_out.recv() {
            Ok(Ok(res)) => res,
            Err(err) => return Some(Err(err.into())),
            Ok(Err(err)) => return Some(Err(err)),
        };

        for node in &tree.nodes {
            if let Some(id) = node.subtree {
                let mut path = path.clone();
                path.push(node.name());
                match self.add_pending(path, id, count) {
                    Ok(_) => {}
                    Err(err) => return Some(Err(err)),
                }
            }
        }
        self.counter[count] -= 1;
        if self.counter[count] == 0 {
            self.p.inc(1);
            self.finished_ids += 1;
        }
        Some(Ok((path, tree)))
    }
}

pub fn merge_trees(
    be: &impl IndexedBackend,
    trees: Vec<Id>,
    cmp: &impl Fn(&Node, &Node) -> Ordering,
    save: &impl Fn(Tree) -> Result<(Id, u64)>,
    summary: &mut SnapshotSummary,
) -> Result<Id> {
    // We store nodes with the index of the tree in an Binary Heap where we sort only by node name
    struct SortedNode(Node, usize);
    impl PartialEq for SortedNode {
        fn eq(&self, other: &Self) -> bool {
            self.0.name == other.0.name
        }
    }
    impl PartialOrd for SortedNode {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            self.0.name.partial_cmp(&other.0.name).map(|o| o.reverse())
        }
    }
    impl Eq for SortedNode {}
    impl Ord for SortedNode {
        fn cmp(&self, other: &Self) -> Ordering {
            self.0.name.cmp(&other.0.name).reverse()
        }
    }

    let mut tree_iters: Vec<_> = trees
        .iter()
        .map(|id| Tree::from_backend(be, *id).map(|tree| tree.into_iter()))
        .collect::<Result<_>>()?;

    // fill Heap with first elements from all trees
    let mut elems = BinaryHeap::new();
    for (num, iter) in tree_iters.iter_mut().enumerate() {
        if let Some(node) = iter.next() {
            elems.push(SortedNode(node, num));
        }
    }

    let mut tree = Tree::new();
    let (mut node, mut num) = match elems.pop() {
        None => {
            let (id, size) = save(tree)?;
            summary.dirs_unmodified += 1;
            summary.total_dirs_processed += 1;
            summary.total_dirsize_processed += size;
            return Ok(id);
        }
        Some(SortedNode(node, num)) => (node, num),
    };

    let mut nodes = Vec::new();
    loop {
        // push next element from tree_iters[0] (if any is left) into BinaryHeap
        if let Some(next_node) = tree_iters[num].next() {
            elems.push(SortedNode(next_node, num));
        }

        match elems.pop() {
            None => {
                // Add node to nodes list
                nodes.push(node);
                // no node left to proceed, merge nodes and quit
                tree.add(merge_nodes(be, nodes, cmp, save, summary)?);
                break;
            }
            Some(SortedNode(new_node, new_num)) if node.name != new_node.name => {
                // Add node to nodes list
                nodes.push(node);
                // next node has other name; merge present nodes
                tree.add(merge_nodes(be, nodes, cmp, save, summary)?);
                nodes = Vec::new();
                // use this node as new node
                (node, num) = (new_node, new_num);
            }
            Some(SortedNode(new_node, new_num)) => {
                // Add node to nodes list
                nodes.push(node);
                // use this node as new node
                (node, num) = (new_node, new_num);
            }
        };
    }
    let (id, size) = save(tree)?;
    if trees.contains(&id) {
        summary.dirs_unmodified += 1;
    } else {
        summary.dirs_changed += 1;
    }
    summary.total_dirs_processed += 1;
    summary.total_dirsize_processed += size;
    Ok(id)
}

fn merge_nodes(
    be: &impl IndexedBackend,
    nodes: Vec<Node>,
    cmp: &impl Fn(&Node, &Node) -> Ordering,
    save: &impl Fn(Tree) -> Result<(Id, u64)>,
    summary: &mut SnapshotSummary,
) -> Result<Node> {
    let trees: Vec<_> = nodes
        .iter()
        .filter(|node| node.is_dir())
        .map(|node| node.subtree.unwrap())
        .collect();

    let mut node = nodes.into_iter().max_by(|n1, n2| cmp(n1, n2)).unwrap();

    // if this is a dir, merge with all other dirs
    if node.is_dir() {
        node.subtree = Some(merge_trees(be, trees, cmp, save, summary)?);
    } else {
        summary.files_unmodified += 1;
        summary.total_files_processed += 1;
        summary.total_bytes_processed += node.meta.size;
    }
    Ok(node)
}
