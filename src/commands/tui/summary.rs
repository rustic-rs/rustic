use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use derive_more::Add;
use itertools::EitherOrBoth;
use ratatui::text::Text;
use rustic_core::{
    DataId, IndexedFull, Progress, Repository, TreeId,
    repofile::{Metadata, Node, Tree},
};

use crate::{
    commands::{ls::Summary, tui::diff::DiffNode},
    helpers::bytes_size_to_string,
};

#[derive(Default)]
pub struct SummaryMap(BTreeMap<TreeId, TreeSummary>);

impl SummaryMap {
    pub fn get(&self, id: &TreeId) -> Option<&TreeSummary> {
        self.0.get(id)
    }

    pub fn compute<S: IndexedFull>(
        &mut self,
        repo: &Repository<S>,
        id: TreeId,
        p: &Progress,
    ) -> Result<()> {
        let _ = TreeSummary::from_tree(repo, id, &mut self.0, p)?;
        Ok(())
    }

    pub fn node_summary(&self, node: &Node) -> Summary {
        if let Some(id) = node.subtree
            && let Some(summary) = self.0.get(&id)
        {
            summary.summary
        } else {
            Summary::from_node(node)
        }
    }

    pub fn compute_statistics<'a, S: IndexedFull>(
        &self,
        nodes: impl IntoIterator<Item = &'a Node>,
        repo: &Repository<S>,
    ) -> Result<Statistics> {
        let builder = nodes
            .into_iter()
            .fold(StatisticsBuilder::default(), |builder, node| {
                builder.append_from_node(node, self)
            });
        builder.build(repo)
    }

    pub fn compute_diff_statistics<S: IndexedFull>(
        &self,
        node: &DiffNode,
        repo: &Repository<S>,
    ) -> Result<DiffStatistics> {
        let stats = match node.map(|n| StatisticsBuilder::default().append_from_node(n, self)) {
            EitherOrBoth::Both(left, right) => {
                let stats_left = Statistics {
                    summary: left.summary,
                    sizes: Sizes::from_blobs(left.blobs.difference(&right.blobs), repo)?,
                };
                let stats_right = Statistics {
                    summary: right.summary,
                    sizes: Sizes::from_blobs(right.blobs.difference(&left.blobs), repo)?,
                };
                let both_sizes = Sizes::from_blobs(left.blobs.intersection(&right.blobs), repo)?;
                return Ok(DiffStatistics {
                    stats: EitherOrBoth::Both(stats_left, stats_right),
                    both_sizes,
                });
            }
            EitherOrBoth::Left(b) => EitherOrBoth::Left(b.build(repo)?),
            EitherOrBoth::Right(b) => EitherOrBoth::Right(b.build(repo)?),
        };
        Ok(DiffStatistics {
            stats,
            ..Default::default()
        })
    }
}

#[derive(Default, Clone)]
pub struct TreeSummary {
    pub id_without_meta: TreeId,
    pub summary: Summary,
    blobs: BTreeSet<DataId>,
    subtrees: Vec<TreeId>,
}

impl TreeSummary {
    fn update(&mut self, other: Self) {
        self.summary += other.summary;
    }

    fn update_from_node(&mut self, node: &Node) {
        for id in node.content.iter().flatten() {
            _ = self.blobs.insert(*id);
        }
        self.summary.update(node);
    }

    pub fn from_tree<S>(
        repo: &'_ Repository<S>,
        id: TreeId,
        summary_map: &mut BTreeMap<TreeId, Self>,
        p: &Progress,
        // Current dir
    ) -> Result<Self>
    where
        S: IndexedFull,
    {
        if let Some(summary) = summary_map.get(&id) {
            return Ok(summary.clone());
        }

        let mut summary = Self::default();

        let tree = repo.get_tree(&id)?;
        let mut tree_without_meta = Tree::default();
        p.inc(1);
        for node in &tree.nodes {
            let mut node_without_meta = Node::new_node(
                node.name().as_os_str(),
                node.node_type.clone(),
                Metadata::default(),
            );
            node_without_meta.content = node.content.clone();
            summary.update_from_node(node);
            if let Some(id) = node.subtree {
                let subtree_summary = Self::from_tree(repo, id, summary_map, p)?;
                node_without_meta.subtree = Some(subtree_summary.id_without_meta);
                summary.update(subtree_summary);
                summary.subtrees.push(id);
            }
            tree_without_meta.nodes.push(node_without_meta);
        }
        let (_, id_without_meta) = tree_without_meta.serialize()?;
        summary.id_without_meta = id_without_meta;

        _ = summary_map.insert(id, summary.clone());
        Ok(summary)
    }
}

#[derive(Default, Clone)]
pub struct StatisticsBuilder<'a> {
    blobs: BTreeSet<&'a DataId>,
    summary: Summary,
}

impl<'a> StatisticsBuilder<'a> {
    fn append_blobs_from_tree_id(&mut self, tree_id: TreeId, summary_map: &'a SummaryMap) {
        if let Some(summary) = summary_map.get(&tree_id) {
            self.blobs.extend(&summary.blobs);
            for id in &summary.subtrees {
                self.append_blobs_from_tree_id(*id, summary_map);
            }
        }
    }
    pub fn append_from_tree(&mut self, tree_id: TreeId, summary_map: &'a SummaryMap) {
        if let Some(summary) = summary_map.get(&tree_id) {
            self.summary += summary.summary;
        }
        self.append_blobs_from_tree_id(tree_id, summary_map);
        self.summary.dirs += 1;
    }
    pub fn append_from_node(mut self, node: &'a Node, summary_map: &'a SummaryMap) -> Self {
        if let Some(tree_id) = &node.subtree {
            self.append_from_tree(*tree_id, summary_map);
        } else {
            self.blobs.extend(node.content.iter().flatten());
            self.summary += summary_map.node_summary(node);
        }
        self
    }
    pub fn build<S: IndexedFull>(self, repo: &'a Repository<S>) -> Result<Statistics> {
        let sizes = Sizes::from_blobs(&self.blobs, repo)?;
        Ok(Statistics {
            summary: self.summary,
            sizes,
        })
    }
}

#[derive(Default)]
pub struct Statistics {
    pub summary: Summary,
    pub sizes: Sizes,
}

impl Statistics {
    pub fn table<'a>(&self, header: String) -> Vec<Vec<Text<'a>>> {
        let row_bytes =
            |title, n: u64| vec![Text::from(title), Text::from(bytes_size_to_string(n))];
        let row_count = |title, n: usize| vec![Text::from(title), Text::from(n.to_string())];

        let mut rows = Vec::new();
        rows.push(vec![Text::from(""), Text::from(header)]);
        rows.push(row_bytes("total size", self.summary.size));
        rows.push(row_count("total files", self.summary.files));
        rows.push(row_count("total dirs", self.summary.dirs));
        rows.push(vec![Text::from(String::new()); 3]);
        rows.push(row_count("total blobs", self.sizes.blobs));
        rows.push(row_bytes(
            "total size after deduplication",
            self.sizes.dedup_size,
        ));
        rows.push(row_bytes("total repoSize", self.sizes.repo_size));
        rows.push(vec![
            Text::from("compression ratio"),
            Text::from(format!("{:.2}", self.sizes.compression_ratio())),
        ]);
        rows
    }
}

#[derive(Default, Add, Clone, Copy)]
pub struct Sizes {
    pub blobs: usize,
    pub repo_size: u64,
    pub dedup_size: u64,
}

impl Sizes {
    pub fn from_blobs<'a, S: IndexedFull>(
        blobs: impl IntoIterator<Item = &'a &'a DataId>,
        repo: &'a Repository<S>,
    ) -> Result<Self> {
        blobs
            .into_iter()
            .map(|id| repo.get_index_entry(*id))
            .try_fold(Self::default(), |sum, ie| -> Result<_> {
                let ie = ie?;
                Ok(Self {
                    blobs: sum.blobs + 1,
                    repo_size: sum.repo_size + u64::from(ie.location.length),
                    dedup_size: sum.dedup_size + u64::from(ie.location.data_length()),
                })
            })
    }

    pub fn compression_ratio(&self) -> f64 {
        self.dedup_size as f64 / self.repo_size as f64
    }
}

pub struct DiffStatistics {
    pub stats: EitherOrBoth<Statistics>,
    pub both_sizes: Sizes,
}

impl DiffStatistics {
    pub fn map<'a, F, T>(&'a self, f: F) -> EitherOrBoth<T>
    where
        F: Fn(&'a Statistics) -> T,
    {
        self.stats.as_ref().map_any(&f, &f)
    }

    pub fn sizes(&self) -> DiffSizes {
        DiffSizes(self.map(|d| d.sizes))
    }
    pub fn total_sizes(&self) -> DiffSizes {
        DiffSizes(self.map(|d| d.sizes + self.both_sizes))
    }
    pub fn both_sizes(&self) -> DiffSizes {
        DiffSizes(self.map(|_| self.both_sizes))
    }
    pub fn summary(&self) -> DiffSummary {
        DiffSummary(self.map(|d| d.summary))
    }
    pub fn table<'a>(&self, header_left: String, header_right: String) -> Vec<Vec<Text<'a>>> {
        fn row_map<'a, T>(
            title: &'static str,
            n: EitherOrBoth<T>,
            map: fn(T) -> String,
        ) -> Vec<Text<'a>> {
            let (left, right) = n.left_and_right();
            vec![
                Text::from(title),
                Text::from(left.map_or_else(String::new, map)),
                Text::from(right.map_or_else(String::new, map)),
            ]
        }

        let row_bytes = |title, n: EitherOrBoth<u64>| row_map(title, n, bytes_size_to_string);
        let row_count = |title, n: EitherOrBoth<usize>| row_map(title, n, |n| n.to_string());

        let mut rows = Vec::new();
        rows.push(vec![
            Text::from(""),
            Text::from(header_left),
            Text::from(header_right),
        ]);
        rows.push(row_bytes("total size", self.summary().size()));
        rows.push(row_count("total files", self.summary().files()));
        rows.push(row_count("total dirs", self.summary().dirs()));
        rows.push(vec![Text::from(String::new()); 3]);
        rows.push(row_count("exclusive blobs", self.sizes().blobs()));
        rows.push(row_count("shared blobs", self.both_sizes().blobs()));
        rows.push(row_count("total blobs", self.total_sizes().blobs()));
        rows.push(vec![Text::from(String::new()); 3]);
        rows.push(row_bytes(
            "exclusive size after deduplication",
            self.sizes().dedup_size(),
        ));
        rows.push(row_bytes(
            "shared size after deduplication",
            self.both_sizes().dedup_size(),
        ));
        rows.push(row_bytes(
            "total size after deduplication",
            self.total_sizes().dedup_size(),
        ));
        rows.push(vec![Text::from(String::new()); 3]);
        rows.push(row_bytes("exclusive repoSize", self.sizes().repo_size()));
        rows.push(row_bytes("shared repoSize", self.both_sizes().repo_size()));
        rows.push(row_bytes("total repoSize", self.total_sizes().repo_size()));
        rows.push(vec![Text::from(String::new()); 3]);
        rows.push(row_map(
            "compression ratio",
            self.total_sizes().compression_ratio(),
            |r| format!("{r:.2}"),
        ));
        rows
    }
}

impl Default for DiffStatistics {
    fn default() -> Self {
        Self {
            stats: EitherOrBoth::Both(Statistics::default(), Statistics::default()),
            both_sizes: Sizes::default(),
        }
    }
}

pub struct DiffSizes(EitherOrBoth<Sizes>);
impl DiffSizes {
    pub fn blobs(&self) -> EitherOrBoth<usize> {
        let map = |s: &Sizes| s.blobs;
        self.0.as_ref().map_any(map, map)
    }
    pub fn repo_size(&self) -> EitherOrBoth<u64> {
        let map = |s: &Sizes| s.repo_size;
        self.0.as_ref().map_any(map, map)
    }
    pub fn dedup_size(&self) -> EitherOrBoth<u64> {
        let map = |s: &Sizes| s.dedup_size;
        self.0.as_ref().map_any(map, map)
    }
    pub fn compression_ratio(&self) -> EitherOrBoth<f64> {
        let map = |s: &Sizes| s.compression_ratio();
        self.0.as_ref().map_any(map, map)
    }
}
pub struct DiffSummary(EitherOrBoth<Summary>);
impl DiffSummary {
    pub fn size(&self) -> EitherOrBoth<u64> {
        let map = |s: &Summary| s.size;
        self.0.as_ref().map_any(map, map)
    }
    pub fn files(&self) -> EitherOrBoth<usize> {
        let map = |s: &Summary| s.files;
        self.0.as_ref().map_any(map, map)
    }
    pub fn dirs(&self) -> EitherOrBoth<usize> {
        let map = |s: &Summary| s.dirs;
        self.0.as_ref().map_any(map, map)
    }
}
