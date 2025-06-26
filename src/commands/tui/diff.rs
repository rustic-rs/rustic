use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    path::PathBuf,
};

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind};
use itertools::{EitherOrBoth, Itertools};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};
use rustic_core::{
    DataId, IndexedFull, Progress, ProgressBars, Repository, TreeId,
    repofile::{Metadata, Node, SnapshotFile, Tree},
};
use style::palette::tailwind;

use crate::{
    commands::{
        diff::{DiffStatistics, NodeDiff},
        snapshots::fill_table,
        tui::widgets::{
            Draw, PopUpPrompt, PopUpText, ProcessEvent, PromptResult, SelectTable, WithBlock,
            popup_prompt, popup_text,
        },
    },
    helpers::bytes_size_to_string,
};

use super::{
    TuiResult,
    widgets::{PopUpTable, popup_table},
};

// the states this screen can be in
enum CurrentScreen {
    Diff,
    ShowHelp(PopUpText),
    SnapshotDetails(PopUpTable),
    PromptExit(PopUpPrompt),
    PromptLeave(PopUpPrompt),
}

const INFO_TEXT: &str =
    "(Esc) quit | (Enter) enter dir | (Backspace) return to parent | (?) show all commands";

const HELP_TEXT: &str = r"
Diff Commands:

          m : toggle ignoring metadata
          d : toggle show only different entries
          s : compute information for (sub-)dirs
          I : show information about snapshots

General Commands:

      q,Esc : exit
      Enter : enter dir
  Backspace : return to parent dir
          ? : show this help page

 ";

#[derive(Clone)]
struct DiffNode(EitherOrBoth<Node>);

impl DiffNode {
    fn only_subtrees(&self) -> Option<Self> {
        let (left, right) = self
            .0
            .clone()
            .map_any(
                |n| n.subtree.is_some().then_some(n),
                |n| n.subtree.is_some().then_some(n),
            )
            .left_and_right();
        match (left.flatten(), right.flatten()) {
            (Some(l), Some(r)) => Some(Self(EitherOrBoth::Both(l, r))),
            (Some(l), None) => Some(Self(EitherOrBoth::Left(l))),
            (None, Some(r)) => Some(Self(EitherOrBoth::Right(r))),
            (None, None) => None,
        }
    }

    fn name(&self) -> OsString {
        self.0.as_ref().reduce(|l, _| l).name()
    }
}

#[derive(Default)]
struct DiffTree {
    nodes: Vec<DiffNode>,
}

impl DiffTree {
    fn from_node<P: ProgressBars, S: IndexedFull>(
        repo: &'_ Repository<P, S>,
        node: &DiffNode,
    ) -> Result<Self> {
        let tree_from_node = |node: Option<&Node>| {
            node.map_or_else(
                || Ok(Tree::default()),
                |node| {
                    node.subtree.map_or_else(
                        || {
                            Ok(Tree {
                                nodes: vec![node.clone()],
                            })
                        },
                        |id| repo.get_tree(&id),
                    )
                },
            )
        };

        let left_tree = tree_from_node(node.0.as_ref().left())?;
        let right_tree = tree_from_node(node.0.as_ref().right())?;
        let nodes = left_tree
            .nodes
            .into_iter()
            .merge_join_by(right_tree.nodes, |node_l, node_r| {
                node_l.name().cmp(&node_r.name())
            })
            .map(DiffNode)
            .collect();
        Ok(Self { nodes })
    }
}

#[derive(Default, Clone)]
struct TreeSummary {
    id_without_meta: TreeId,
    blobs: BTreeSet<DataId>,
    size: u64,
}

impl TreeSummary {
    fn update(&mut self, mut other: Self) {
        self.blobs.append(&mut other.blobs);
        self.size += other.size;
    }

    fn update_from_node(&mut self, node: &Node) {
        for id in node.content.iter().flatten() {
            _ = self.blobs.insert(*id);
        }
        self.size += node.meta.size;
    }

    fn from_repo<P, S>(
        repo: &'_ Repository<P, S>,
        ids: &DiffNode,
        summary_map: &mut BTreeMap<TreeId, Self>,
        p: &impl Progress,
    ) -> Result<()>
    where
        P: ProgressBars,
        S: IndexedFull,
    {
        let (left, right) = ids.0.as_ref().left_and_right();
        if let Some(node) = left {
            if let Some(id) = node.subtree {
                let _ = Self::from_tree(repo, id, summary_map, p)?;
            }
        }
        if let Some(node) = right {
            if let Some(id) = node.subtree {
                let _ = Self::from_tree(repo, id, summary_map, p)?;
            }
        }
        Ok(())
    }

    fn from_tree<P, S>(
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

pub struct Diff<'a, P, S> {
    current_screen: CurrentScreen,
    table: WithBlock<SelectTable>,
    repo: &'a Repository<P, S>,
    snapshot_left: SnapshotFile,
    snapshot_right: SnapshotFile,
    path_left: PathBuf,
    path_right: PathBuf,
    trees: Vec<(DiffTree, DiffNode, usize)>, // Stack of parent trees with position
    tree: DiffTree,
    node: DiffNode,
    summary_map: BTreeMap<TreeId, TreeSummary>,
    ignore_metadata: bool,
    ignore_identical: bool,
}

pub enum DiffResult {
    Exit,
    Return,
    None,
}

impl TuiResult for DiffResult {
    fn exit(&self) -> bool {
        !matches!(self, Self::None)
    }
}

impl<'a, P: ProgressBars, S: IndexedFull> Diff<'a, P, S> {
    pub fn new(
        repo: &'a Repository<P, S>,
        snap_left: SnapshotFile,
        snap_right: SnapshotFile,
        path_left: &str,
        path_right: &str,
    ) -> Result<Self> {
        let header = [
            "Name",
            "Time",
            "Size",
            "- RepoSize",
            "Time",
            "Size",
            "+ RepoSize",
        ]
        .into_iter()
        .map(Text::from)
        .collect();

        let left = repo.node_from_snapshot_and_path(&snap_left, path_left)?;
        let right = repo.node_from_snapshot_and_path(&snap_right, path_right)?;
        let node = DiffNode(EitherOrBoth::Both(left, right));

        let mut tree = DiffTree::from_node(repo, &node)?;
        let mut app = Self {
            current_screen: CurrentScreen::Diff,
            table: WithBlock::new(SelectTable::new(header), Block::new()),
            repo,
            snapshot_left: snap_left,
            snapshot_right: snap_right,
            path_left: path_left.parse()?,
            path_right: path_right.parse()?,
            trees: Vec::new(),
            tree: DiffTree::default(),
            node,
            summary_map: BTreeMap::new(),
            ignore_metadata: true,
            ignore_identical: true,
        };
        tree.nodes.retain(|node| app.show_node(node));
        app.tree = tree;
        app.update_table();
        Ok(app)
    }

    fn node_changed(&self, node: &DiffNode) -> NodeDiff {
        let (left, right) = node.0.as_ref().left_and_right();
        let mut changed = NodeDiff::from(left, right, |left, right| {
            if left.content != right.content {
                return false;
            }
            if self.ignore_metadata {
                if let (Some(id_left), Some(id_right)) = (left.subtree, right.subtree) {
                    if let (Some(summary_left), Some(summary_right)) = (
                        self.summary_map.get(&id_left),
                        self.summary_map.get(&id_right),
                    ) {
                        return summary_left.id_without_meta == summary_right.id_without_meta;
                    }
                }
            }
            left.subtree == right.subtree
        });
        if self.ignore_metadata {
            changed = changed.ignore_metadata();
        }
        changed
    }

    fn show_node(&self, node: &DiffNode) -> bool {
        !self.ignore_identical || !self.node_changed(node).is_identical()
    }

    fn ls_row(&self, node: &DiffNode, stat: &mut DiffStatistics) -> Vec<Text<'static>> {
        let node_info = |node: &Node| {
            let size = node.subtree.map_or(node.meta.size, |id| {
                self.summary_map
                    .get(&id)
                    .map_or(node.meta.size, |summary| summary.size)
            });
            (
                bytes_size_to_string(size),
                node.meta.mtime.map_or_else(
                    || "?".to_string(),
                    |t| format!("{}", t.format("%Y-%m-%d %H:%M:%S")),
                ),
            )
        };

        let compute_diff = |blobs1: &BTreeSet<&DataId>, blobs2: &BTreeSet<&DataId>| {
            if blobs1.is_empty() {
                String::new()
            } else {
                blobs1
                    .difference(blobs2)
                    .map(|id| self.repo.get_index_entry(*id))
                    .try_fold(0u64, |sum, b| -> Result<_> {
                        Ok(sum + u64::from(b?.length))
                    })
                    .ok()
                    .map_or("?".to_string(), bytes_size_to_string)
            }
        };

        let (left, right) = node.0.as_ref().left_and_right();
        let left_blobs = left.map_or_else(BTreeSet::new, |node| {
            if let Some(id) = node.subtree {
                if let Some(summary) = self.summary_map.get(&id) {
                    return summary.blobs.iter().collect();
                }
            }
            node.content.iter().flatten().collect()
        });
        let right_blobs = right.map_or_else(BTreeSet::new, |node| {
            if let Some(id) = node.subtree {
                if let Some(summary) = self.summary_map.get(&id) {
                    return summary.blobs.iter().collect();
                }
            }
            node.content.iter().flatten().collect()
        });
        let left_only = compute_diff(&left_blobs, &right_blobs);
        let right_only = compute_diff(&right_blobs, &left_blobs);

        let changed = self.node_changed(node);
        stat.apply(changed);
        let name = node.name();
        let name = format!("{changed} {}", name.to_string_lossy());
        let (left_size, left_mtime) = match &node.0 {
            EitherOrBoth::Left(node) | EitherOrBoth::Both(node, _) => node_info(node),
            _ => (String::new(), String::new()),
        };
        let (right_size, right_mtime) = match &node.0 {
            EitherOrBoth::Right(node) | EitherOrBoth::Both(_, node) => node_info(node),
            _ => (String::new(), String::new()),
        };
        [
            name,
            left_mtime,
            left_size,
            left_only,
            right_mtime,
            right_size,
            right_only,
        ]
        .into_iter()
        .map(Text::from)
        .collect()
    }

    pub fn update_table(&mut self) {
        let mut stat = DiffStatistics::default();
        let old_selection = if self.tree.nodes.is_empty() {
            None
        } else {
            Some(self.table.widget.selected().unwrap_or_default())
        };
        let mut rows = Vec::new();
        for node in &self.tree.nodes {
            let row = self.ls_row(node, &mut stat);
            rows.push(row);
        }

        self.table.widget.set_content(rows, 1);

        self.table.block = Block::new()
            .borders(Borders::BOTTOM | Borders::TOP)
            .title_bottom(format!(
                "total: {}, files: {}, dirs: {}; {} equal, {} metadata",
                self.tree.nodes.len(),
                stat.files,
                stat.dirs,
                if self.ignore_identical {
                    "hide"
                } else {
                    "show"
                },
                if self.ignore_metadata {
                    "with"
                } else {
                    "without"
                }
            ))
            .title(format!(
                "{} | {}",
                if self.node.0.has_left() {
                    format!("{}:{}", self.snapshot_left.id, self.path_left.display())
                } else {
                    format!("({})", self.snapshot_left.id)
                },
                if self.node.0.has_right() {
                    format!("{}:{}", self.snapshot_right.id, self.path_right.display())
                } else {
                    format!("({})", self.snapshot_right.id)
                },
            ))
            .title_alignment(Alignment::Center);
        self.table.widget.select(old_selection);
    }

    pub fn enter(&mut self) -> Result<()> {
        if let Some(idx) = self.table.widget.selected() {
            let node = &self.tree.nodes[idx];
            if let Some(node) = node.only_subtrees() {
                self.path_left.push(node.name());
                self.path_right.push(node.name());
                let tree = std::mem::take(&mut self.tree);
                self.trees.push((tree, self.node.clone(), idx));
                let mut tree = DiffTree::from_node(self.repo, &node)?;
                tree.nodes.retain(|node| self.show_node(node));
                self.tree = tree;
                self.node = node;
                self.table.widget.set_to(0);
                self.update_table();
            }
        }
        Ok(())
    }

    pub fn in_root(&self) -> bool {
        self.trees.is_empty()
    }

    pub fn goback(&mut self) {
        _ = self.path_left.pop();
        _ = self.path_right.pop();
        if let Some((tree, node, idx)) = self.trees.pop() {
            self.tree = tree;
            self.node = node;
            self.table.widget.set_to(idx);
            self.update_table();
        }
    }

    pub fn toggle_ignore_metadata(&mut self) {
        self.ignore_metadata = !self.ignore_metadata;
        self.update_table();
    }

    pub fn toggle_ignore_identical(&mut self) -> Result<()> {
        self.ignore_identical = !self.ignore_identical;

        let mut tree = DiffTree::from_node(self.repo, &self.node)?;
        tree.nodes.retain(|node| self.show_node(node));
        self.tree = tree;

        self.update_table();
        Ok(())
    }

    pub fn compute_summary(&mut self) -> Result<()> {
        let pb = self.repo.progress_bars();
        let p = pb.progress_counter("computing (sub)-dir information");
        TreeSummary::from_repo(self.repo, &self.node, &mut self.summary_map, &p)?;
        p.finish();
        self.update_table();
        Ok(())
    }

    pub fn snapshot_details(&self) -> PopUpTable {
        let mut rows = Vec::new();
        let mut rows_right = Vec::new();
        fill_table(&self.snapshot_left, |title, value| {
            rows.push(vec![Text::from(title.to_string()), Text::from(value)]);
        });
        fill_table(&self.snapshot_right, |_, value| {
            rows_right.push(Text::from(value));
        });
        for (row, right) in rows.iter_mut().zip(rows_right) {
            row.push(right);
        }
        popup_table("snapshot details", rows)
    }
}

impl<'a, P: ProgressBars, S: IndexedFull> ProcessEvent for Diff<'a, P, S> {
    type Result = Result<DiffResult>;
    fn input(&mut self, event: Event) -> Result<DiffResult> {
        use KeyCode::{Backspace, Char, Enter, Esc, Left, Right};
        match &mut self.current_screen {
            CurrentScreen::Diff => match event {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    Enter | Right => self.enter()?,
                    Backspace | Left => {
                        if self.in_root() {
                            self.current_screen = CurrentScreen::PromptLeave(popup_prompt(
                                "leave diff",
                                "do you want to leave the diff view? (y/n)".into(),
                            ));
                        } else {
                            self.goback();
                        }
                    }
                    Esc | Char('q') => {
                        self.current_screen = CurrentScreen::PromptExit(popup_prompt(
                            "exit rustic",
                            "do you want to exit? (y/n)".into(),
                        ));
                    }
                    Char('?') => {
                        self.current_screen =
                            CurrentScreen::ShowHelp(popup_text("help", HELP_TEXT.into()));
                    }
                    Char('m') => self.toggle_ignore_metadata(),
                    Char('d') => self.toggle_ignore_identical()?,
                    Char('s') => self.compute_summary()?,
                    Char('I') => {
                        self.current_screen =
                            CurrentScreen::SnapshotDetails(self.snapshot_details());
                    }
                    _ => self.table.input(event),
                },
                _ => {}
            },
            CurrentScreen::SnapshotDetails(_) | CurrentScreen::ShowHelp(_) => match event {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if matches!(key.code, Char('q' | ' ' | 'I' | '?') | Esc | Enter) {
                        self.current_screen = CurrentScreen::Diff;
                    }
                }
                _ => {}
            },
            CurrentScreen::PromptExit(prompt) => match prompt.input(event) {
                PromptResult::Ok => return Ok(DiffResult::Exit),
                PromptResult::Cancel => self.current_screen = CurrentScreen::Diff,
                PromptResult::None => {}
            },
            CurrentScreen::PromptLeave(prompt) => match prompt.input(event) {
                PromptResult::Ok => return Ok(DiffResult::Return),
                PromptResult::Cancel => self.current_screen = CurrentScreen::Diff,
                PromptResult::None => {}
            },
        }
        Ok(DiffResult::None)
    }
}

impl<'a, P: ProgressBars, S: IndexedFull> Draw for Diff<'a, P, S> {
    fn draw(&mut self, area: Rect, f: &mut Frame<'_>) {
        let rects = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);

        // draw the table
        self.table.draw(rects[0], f);

        // draw the footer
        let buffer_bg = tailwind::SLATE.c950;
        let row_fg = tailwind::SLATE.c200;
        let info_footer = Paragraph::new(Line::from(INFO_TEXT))
            .style(Style::new().fg(row_fg).bg(buffer_bg))
            .centered();
        f.render_widget(info_footer, rects[1]);

        // draw popups
        match &mut self.current_screen {
            CurrentScreen::Diff => {}
            CurrentScreen::SnapshotDetails(popup) => popup.draw(area, f),
            CurrentScreen::ShowHelp(popup) => popup.draw(area, f),
            CurrentScreen::PromptExit(popup) | CurrentScreen::PromptLeave(popup) => {
                popup.draw(area, f);
            }
        }
    }
}
