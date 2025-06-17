use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind};
use itertools::{EitherOrBoth, Itertools};
use log::debug;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};
use rustic_core::{
    DataId, IndexedFull, ProgressBars, Repository, TreeId,
    repofile::{Node, SnapshotFile, Tree},
};
use style::palette::tailwind;

use crate::commands::{
    diff::NodeDiff,
    tui::widgets::{
        Draw, PopUpPrompt, PopUpText, ProcessEvent, PromptResult, SelectTable, WithBlock,
        popup_prompt, popup_text,
    },
};

// the states this screen can be in
enum CurrentScreen {
    Snapshot,
    ShowHelp(PopUpText),
    PromptExit(PopUpPrompt),
}

const INFO_TEXT: &str =
    "(Esc) quit | (Enter) enter dir | (Backspace) return to parent | (?) show all commands";

const HELP_TEXT: &str = r"
Diff Commands:

          m : toggle ignoring metadata
          d : toggle show only different entries
          s : compute diff for (sub-)dirs

General Commands:

      q,Esc : exit
      Enter : enter dir
  Backspace : return to parent dir
          ? : show this help page

 ";

#[derive(Default)]
struct DiffTree {
    nodes: Vec<EitherOrBoth<Node, Node>>,
}

impl DiffTree {
    fn from_trees<P: ProgressBars, S: IndexedFull>(
        repo: &'_ Repository<P, S>,
        left: Option<TreeId>,
        right: Option<TreeId>,
    ) -> Result<Self> {
        let left_tree = if let Some(left) = left {
            repo.get_tree(&left)?
        } else {
            Tree::default()
        };
        let right_tree = if let Some(right) = right {
            repo.get_tree(&right)?
        } else {
            Tree::default()
        };
        let nodes = left_tree
            .nodes
            .into_iter()
            .merge_join_by(right_tree.nodes, |node_l, node_r| {
                node_l.name().cmp(&node_r.name())
            })
            .collect();
        Ok(Self { nodes })
    }

    fn left(&self) -> Tree {
        let nodes = self.nodes.iter().filter_map(|n| n.clone().left()).collect();
        Tree { nodes }
    }
    fn right(&self) -> Tree {
        let nodes = self
            .nodes
            .iter()
            .filter_map(|n| n.clone().right())
            .collect();
        Tree { nodes }
    }
}

#[derive(Default, Clone)]
struct TreeSummary {
    blobs: BTreeSet<DataId>,
    size: u64,
}

impl TreeSummary {
    fn update(&mut self, mut other: Self) {
        self.blobs.append(&mut other.blobs);
        self.size += other.size;
    }

    fn from_repo<P, S>(
        repo: &'_ Repository<P, S>,
        tree: &Tree,
        id: TreeId,
        blobs_map: &mut BTreeMap<TreeId, Self>,
    ) -> Result<Self>
    where
        S: IndexedFull,
    {
        if let Some(summary) = blobs_map.get(&id) {
            return Ok(summary.clone());
        }

        let mut summary = Self::default();
        for node in &tree.nodes {
            for id in node.content.iter().flatten() {
                _ = summary.blobs.insert(*id);
            }
            summary.size += node.meta.size;
            if let Some(id) = node.subtree {
                let tree = repo.get_tree(&id)?;
                summary.update(Self::from_repo(repo, &tree, id, blobs_map)?);
            }
        }
        _ = blobs_map.insert(id, summary.clone());
        Ok(summary)
    }
}

pub(crate) struct Diff<'a, P, S> {
    current_screen: CurrentScreen,
    table: WithBlock<SelectTable>,
    repo: &'a Repository<P, S>,
    snapshot: SnapshotFile,
    path: PathBuf,
    trees: Vec<(DiffTree, Option<TreeId>, Option<TreeId>, usize)>, // Stack of parent trees with position
    tree: DiffTree,
    left: Option<TreeId>,
    right: Option<TreeId>,
    blobs_map: BTreeMap<TreeId, TreeSummary>,
    ignore_metadata: bool,
    ignore_identical: bool,
}

pub enum DiffResult {
    Exit,
    Return,
    None,
}

impl<'a, P: ProgressBars, S: IndexedFull> Diff<'a, P, S> {
    pub fn new(repo: &'a Repository<P, S>, left: TreeId, right: TreeId) -> Result<Self> {
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

        let mut app = Self {
            current_screen: CurrentScreen::Snapshot,
            table: WithBlock::new(SelectTable::new(header), Block::new()),
            repo,
            snapshot: SnapshotFile::default(),
            path: PathBuf::new(),
            trees: Vec::new(),
            tree: DiffTree::default(),
            left: Some(left),
            right: Some(right),
            blobs_map: BTreeMap::new(),
            ignore_metadata: true,
            ignore_identical: true,
        };
        let mut tree = DiffTree::from_trees(repo, Some(left), Some(right))?;
        tree.nodes.retain(|node| app.show_node(node));
        app.tree = tree;
        app.update_table();
        Ok(app)
    }

    fn node_changed(&self, node: &EitherOrBoth<Node, Node>) -> NodeDiff {
        let (left, right) = node.as_ref().left_and_right();
        let mut changed = NodeDiff::from(left, right, |n1, n2| n1.content == n2.content);
        if self.ignore_metadata {
            changed = changed.ignore_metadata();
        }
        changed
    }

    fn show_node(&self, node: &EitherOrBoth<Node, Node>) -> bool {
        !self.ignore_identical || !self.node_changed(node).is_identical()
    }

    fn ls_row(&self, node: &EitherOrBoth<Node, Node>) -> Vec<Text<'static>> {
        let node_info = |node: &Node| {
            let size = node.subtree.map_or(node.meta.size, |id| {
                self.blobs_map
                    .get(&id)
                    .map_or(node.meta.size, |summary| summary.size)
            });
            (
                size.to_string(),
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
                    .map_or("?".to_string(), |s| s.to_string())
            }
        };

        let (left, right) = node.as_ref().left_and_right();
        let left_blobs = left.map_or_else(BTreeSet::new, |node| {
            if let Some(id) = node.subtree {
                if let Some(summary) = self.blobs_map.get(&id) {
                    return summary.blobs.iter().collect();
                }
            }
            node.content.iter().flatten().collect()
        });
        let right_blobs = right.map_or_else(BTreeSet::new, |node| {
            if let Some(id) = node.subtree {
                if let Some(summary) = self.blobs_map.get(&id) {
                    return summary.blobs.iter().collect();
                }
            }
            node.content.iter().flatten().collect()
        });
        let left_only = compute_diff(&left_blobs, &right_blobs);
        let right_only = compute_diff(&right_blobs, &left_blobs);

        let changed = self.node_changed(node);
        let name = node.as_ref().reduce(|l, _| l).name();
        let name = format!("{changed} {}", name.to_string_lossy());
        let (left_size, left_mtime) = match node {
            EitherOrBoth::Left(node) | EitherOrBoth::Both(node, _) => node_info(node),
            _ => (String::new(), String::new()),
        };
        let (right_size, right_mtime) = match node {
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
        let old_selection = if self.tree.nodes.is_empty() {
            None
        } else {
            Some(self.table.widget.selected().unwrap_or_default())
        };
        let mut rows = Vec::new();
        for node in &self.tree.nodes {
            let row = self.ls_row(node);
            rows.push(row);
        }

        self.table.widget.set_content(rows, 1);

        self.table.block = Block::new()
            .borders(Borders::BOTTOM | Borders::TOP)
            .title(format!("{}:{}", self.snapshot.id, self.path.display()))
            .title_alignment(Alignment::Center);
        self.table.widget.select(old_selection);
    }

    pub fn enter(&mut self) -> Result<()> {
        if let Some(idx) = self.table.widget.selected() {
            let node = &self.tree.nodes[idx];
            let (new_left, new_right) = match node {
                EitherOrBoth::Left(left) => (left.subtree, None),
                EitherOrBoth::Right(right) => (None, right.subtree),
                EitherOrBoth::Both(left, right) => (left.subtree, right.subtree),
            };
            debug!("{new_left:?}, {new_right:?}");
            if (new_left, new_right) != (None, None) {
                let tree = std::mem::take(&mut self.tree);
                self.trees.push((tree, self.left, self.right, idx));
                let mut tree = DiffTree::from_trees(self.repo, new_left, new_right)?;
                tree.nodes.retain(|node| self.show_node(node));
                self.tree = tree;
                self.left = new_left;
                self.right = new_right;
                self.table.widget.set_to(0);
                self.update_table();
            }
        }
        Ok(())
    }

    pub fn goback(&mut self) -> bool {
        _ = self.path.pop();
        if let Some((tree, left, right, idx)) = self.trees.pop() {
            self.tree = tree;
            self.left = left;
            self.right = right;
            self.table.widget.set_to(idx);
            self.update_table();
            false
        } else {
            true
        }
    }

    pub fn toggle_ignore_metadata(&mut self) {
        self.ignore_metadata = !self.ignore_metadata;
        self.update_table();
    }

    pub fn toggle_ignore_identical(&mut self) -> Result<()> {
        self.ignore_identical = !self.ignore_identical;

        let mut tree = DiffTree::from_trees(self.repo, self.left, self.right)?;
        tree.nodes.retain(|node| self.show_node(node));
        self.tree = tree;

        self.update_table();
        Ok(())
    }

    pub fn compute_blobs(&mut self) {
        if let Some(left) = self.left {
            let _ = TreeSummary::from_repo(self.repo, &self.tree.left(), left, &mut self.blobs_map)
                .unwrap();
        }
        if let Some(right) = self.right {
            let _ =
                TreeSummary::from_repo(self.repo, &self.tree.right(), right, &mut self.blobs_map)
                    .unwrap();
        }
        self.update_table();
    }

    pub fn input(&mut self, event: Event) -> Result<DiffResult> {
        use KeyCode::{Backspace, Char, Enter, Esc, Left, Right};
        match &mut self.current_screen {
            CurrentScreen::Snapshot => match event {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    Enter | Right => self.enter()?,
                    Backspace | Left => {
                        if self.goback() {
                            return Ok(DiffResult::Return);
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
                    Char('s') => self.compute_blobs(),
                    _ => self.table.input(event),
                },
                _ => {}
            },
            CurrentScreen::ShowHelp(_) => match event {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if matches!(key.code, Char('q' | ' ' | '?') | Esc | Enter) {
                        self.current_screen = CurrentScreen::Snapshot;
                    }
                }
                _ => {}
            },
            CurrentScreen::PromptExit(prompt) => match prompt.input(event) {
                PromptResult::Ok => return Ok(DiffResult::Exit),
                PromptResult::Cancel => self.current_screen = CurrentScreen::Snapshot,
                PromptResult::None => {}
            },
        }
        Ok(DiffResult::None)
    }

    pub fn draw(&mut self, area: Rect, f: &mut Frame<'_>) {
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
            CurrentScreen::Snapshot => {}
            CurrentScreen::ShowHelp(popup) => popup.draw(area, f),
            CurrentScreen::PromptExit(popup) => popup.draw(area, f),
        }
    }
}
