use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use rustic_core::{
    DataId, IndexedFull, Progress, Repository, TreeId,
    repofile::{Metadata, Node, Tree},
};

use crate::{commands::ls::Summary, helpers::bytes_size_to_string};

#[derive(Default)]
pub struct SummaryMap(BTreeMap<TreeId, TreeSummary>);

impl SummaryMap {
    pub fn get(&self, id: &TreeId) -> Option<&TreeSummary> {
        self.0.get(id)
    }

    pub fn compute<P, S: IndexedFull>(
        &mut self,
        repo: &Repository<P, S>,
        id: TreeId,
        p: &impl Progress,
    ) -> Result<()> {
        let _ = TreeSummary::from_tree(repo, id, &mut self.0, p)?;
        Ok(())
    }
}

#[derive(Default, Clone)]
pub struct TreeSummary {
    pub id_without_meta: TreeId,
    pub blobs: BlobInfo,
    pub summary: Summary,
}

impl TreeSummary {
    fn update(&mut self, mut other: Self) {
        self.blobs.0.append(&mut other.blobs.0);
        self.summary += other.summary;
    }

    fn update_from_node(&mut self, node: &Node) {
        for id in node.content.iter().flatten() {
            _ = self.blobs.0.insert(*id);
        }
        self.summary.update(node);
    }

    pub fn from_tree<P, S>(
        repo: &'_ Repository<P, S>,
        id: TreeId,
        summary_map: &mut BTreeMap<TreeId, Self>,
        p: &impl Progress,
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
pub struct BlobInfo(BTreeSet<DataId>);

impl BlobInfo {
    pub fn as_ref(&self) -> BlobInfoRef<'_> {
        BlobInfoRef(self.0.iter().collect())
    }
}

pub struct BlobInfoRef<'a>(BTreeSet<&'a DataId>);
impl<'a> BlobInfoRef<'a> {
    pub fn from_node_or_map(node: &'a Node, summary_map: &'a SummaryMap) -> Self {
        node.subtree.map_or_else(
            || Self::from_node(node),
            |id| {
                summary_map
                    .get(&id)
                    .map_or_else(|| Self::from_node(node), |summary| summary.blobs.as_ref())
            },
        )
    }
    fn from_node(node: &'a Node) -> Self {
        Self(node.content.iter().flatten().collect())
    }

    pub fn text_diff<P, S: IndexedFull>(
        blobs1: &Option<Self>,
        blobs2: &Option<Self>,
        repo: &'a Repository<P, S>,
    ) -> String {
        if let (Some(blobs1), Some(blobs2)) = (blobs1, blobs2) {
            blobs1
                .0
                .difference(&blobs2.0)
                .map(|id| repo.get_index_entry(*id))
                .try_fold(0u64, |sum, b| -> Result<_> {
                    Ok(sum + u64::from(b?.length))
                })
                .ok()
                .map_or("?".to_string(), bytes_size_to_string)
        } else {
            String::new()
        }
    }
}
