use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use rustic_core::{
    DataId, IndexedFull, Progress, Repository, TreeId,
    repofile::{Metadata, Node, Tree},
};

use crate::commands::ls::Summary;

#[derive(Default, Clone)]
pub struct TreeSummary {
    pub id_without_meta: TreeId,
    pub blobs: BTreeSet<DataId>,
    pub summary: Summary,
}

impl TreeSummary {
    fn update(&mut self, mut other: Self) {
        self.blobs.append(&mut other.blobs);
        self.summary += other.summary;
    }

    fn update_from_node(&mut self, node: &Node) {
        for id in node.content.iter().flatten() {
            _ = self.blobs.insert(*id);
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
