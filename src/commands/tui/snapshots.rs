use std::{collections::BTreeSet, iter::once, mem, str::FromStr};

use anyhow::Result;
use chrono::Local;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use itertools::Itertools;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};
use rustic_core::{
    IndexedFull, ProgressBars, Repository, SnapshotGroup, SnapshotGroupCriterion, StringList,
    repofile::{DeleteOption, SnapshotFile},
};
use style::palette::tailwind;

use crate::{
    commands::{
        snapshots::{fill_table, snap_to_table},
        tui::{
            diff::{Diff, DiffResult},
            ls::{Snapshot, SnapshotResult},
            tree::{Tree, TreeIterItem, TreeNode},
            widgets::{
                Draw, PopUpInput, PopUpPrompt, PopUpTable, PopUpText, ProcessEvent, PromptResult,
                SelectTable, TextInputResult, WithBlock, popup_input, popup_prompt, popup_table,
                popup_text,
            },
        },
    },
    filtering::SnapshotFilter,
};

// the states this screen can be in
enum CurrentScreen<'a, P, S> {
    Snapshots,
    ShowHelp(PopUpText),
    SnapshotDetails(PopUpTable),
    EnterLabel(PopUpInput),
    EnterDescription(PopUpInput),
    EnterAddTags(PopUpInput),
    EnterSetTags(PopUpInput),
    EnterRemoveTags(PopUpInput),
    EnterFilter(PopUpInput),
    PromptWrite(PopUpPrompt),
    PromptExit(PopUpPrompt),
    Dir(Box<Snapshot<'a, P, S>>),
    Diff(Box<Diff<'a, P, S>>),
}

// status of each snapshot
#[derive(Clone, Copy, Default)]
struct SnapStatus {
    marked: bool,
    modified: bool,
    to_forget: bool,
}

impl SnapStatus {
    fn toggle_mark(&mut self) {
        self.marked = !self.marked;
    }
}

#[derive(Debug)]
enum View {
    Filter,
    All,
    Marked,
    Modified,
}

#[derive(PartialEq, Eq)]
enum SnapshotNode {
    Group(SnapshotGroup),
    Snap(usize),
}

const INFO_TEXT: &str = "(Esc) quit | (F5) reload snapshots | (Enter) show contents | (v) toggle view | (i) show snapshot | (?) show all commands";

const HELP_TEXT: &str = r"General Commands:
  q, Esc : exit
      F5 : re-read all snapshots from repository
   Enter : show snapshot contents
       v : toggle snapshot view [Filtered -> All -> Marked -> Modified]
       V : modify filter to use     
  Ctrl-v : reset filter
       i : show detailed snapshot information for selected snapshot
       w : write modified snapshots and delete snapshots to-forget
       ? : show this help page
 
 Commands for marking snapshot(s):
 
       x : toggle marking for selected snapshot
       X : toggle markings for all snapshots
  Ctrl-x : clear all markings
 
 Commands applied to marked snapshot(s) (selected if none marked):
 
       f : toggle to-forget for snapshot(s)
  Ctrl-f : clear to-forget for snapshot(s)
       l : set label for snapshot(s)
  Ctrl-l : remove label for snapshot(s)
       d : set description for snapshot(s)
  Ctrl-d : remove description for snapshot(s)
       D : diff snapshots if 2 snapshots are selected
       t : add tag(s) for snapshot(s)
  Ctrl-t : remove all tags for snapshot(s)
       s : set tag(s) for snapshot(s)
       r : remove tag(s) for snapshot(s)
       p : set delete protection for snapshot(s)
  Ctrl-p : remove delete protection for snapshot(s)
";

pub(crate) struct Snapshots<'a, P, S> {
    current_screen: CurrentScreen<'a, P, S>,
    current_view: View,
    table: WithBlock<SelectTable>,
    repo: &'a Repository<P, S>,
    snaps_status: Vec<SnapStatus>,
    snapshots: Vec<SnapshotFile>,
    original_snapshots: Vec<SnapshotFile>,
    filtered_snapshots: Vec<usize>,
    tree: Tree<SnapshotNode, usize>,
    filter: SnapshotFilter,
    default_filter: SnapshotFilter,
    group_by: SnapshotGroupCriterion,
}

impl<'a, P: ProgressBars, S: IndexedFull> Snapshots<'a, P, S> {
    pub fn new(
        repo: &'a Repository<P, S>,
        filter: SnapshotFilter,
        group_by: SnapshotGroupCriterion,
    ) -> Result<Self> {
        let header = [
            "", " ID", "Time", "Host", "Label", "Tags", "Paths", "Files", "Dirs", "Size",
        ]
        .into_iter()
        .map(Text::from)
        .collect();

        let mut app = Self {
            current_screen: CurrentScreen::Snapshots,
            current_view: View::Filter,
            table: WithBlock::new(SelectTable::new(header), Block::new()),
            repo,
            snaps_status: Vec::new(),
            original_snapshots: Vec::new(),
            snapshots: Vec::new(),
            filtered_snapshots: Vec::new(),
            tree: Tree::Leaf(0),
            default_filter: filter.clone(),
            filter,
            group_by,
        };
        app.reread()?;
        Ok(app)
    }

    fn selected_tree(&self) -> Option<TreeIterItem<'_, SnapshotNode, usize>> {
        self.table
            .widget
            .selected()
            .and_then(|selected| self.tree.iter_open().nth(selected))
    }

    fn selected_tree_mut(&mut self) -> Option<&mut Tree<SnapshotNode, usize>> {
        self.table
            .widget
            .selected()
            .and_then(|selected| self.tree.nth_mut(selected))
    }

    fn snap_idx(&self) -> Vec<usize> {
        self.selected_tree()
            .iter()
            .flat_map(|item| item.tree.iter().map(|item| item.tree))
            .filter_map(|tree| tree.leaf_data().copied())
            .collect()
    }

    fn selected_snapshot(&self) -> Option<&SnapshotFile> {
        self.selected_tree().map(|tree_info| match tree_info.tree {
            Tree::Leaf(index)
            | Tree::Node(TreeNode {
                data: SnapshotNode::Snap(index),
                ..
            }) => Some(&self.snapshots[*index]),
            _ => None,
        })?
    }

    pub fn has_mark(&self) -> bool {
        self.snaps_status.iter().any(|s| s.marked)
    }

    pub fn has_modified(&self) -> bool {
        self.snaps_status.iter().any(|s| s.modified)
    }

    pub fn toggle_view_mark(&mut self) {
        match self.current_view {
            View::Filter => self.current_view = View::All,
            View::All => {
                self.current_view = View::Marked;
                if !self.has_mark() {
                    self.toggle_view_mark();
                }
            }
            View::Marked => {
                self.current_view = View::Modified;
                if !self.has_modified() {
                    self.toggle_view_mark();
                }
            }
            View::Modified => self.current_view = View::Filter,
        }
    }

    pub fn toggle_view(&mut self) {
        self.toggle_view_mark();
        self.apply_view();
    }

    pub fn apply_view(&mut self) {
        // select snapshots to show
        self.filtered_snapshots = self
            .snapshots
            .iter()
            .enumerate()
            .zip(self.snaps_status.iter())
            .filter_map(|((i, sn), status)| {
                match self.current_view {
                    View::All => true,
                    View::Filter => self.filter.matches(sn),
                    View::Marked => status.marked,
                    View::Modified => status.modified,
                }
                .then_some(i)
            })
            .collect();
        self.create_tree();
    }

    pub fn create_tree(&mut self) {
        // remember current snapshot index
        let old_tree = self.selected_tree().map(|t| t.tree);

        let mut result = Vec::new();
        for (group, snaps) in &self
            .filtered_snapshots
            .iter()
            .chunk_by(|i| SnapshotGroup::from_snapshot(&self.snapshots[**i], self.group_by))
        {
            let mut same_id_group = Vec::new();
            for (_, s) in &snaps.into_iter().chunk_by(|i| self.snapshots[**i].tree) {
                let leafs: Vec<_> = s.map(|i| Tree::leaf(*i)).collect();
                let first = leafs[0].leaf_data().unwrap(); // Cannot be None as leafs[0] is a leaf!
                if leafs.len() == 1 {
                    same_id_group.push(Tree::leaf(*first));
                } else {
                    same_id_group.push(Tree::node(SnapshotNode::Snap(*first), false, leafs));
                }
            }
            result.push(Tree::node(SnapshotNode::Group(group), false, same_id_group));
        }
        let tree = Tree::node(SnapshotNode::Snap(0), true, result);

        let len = tree.iter_open().count();
        let selected = if len == 0 {
            None
        } else {
            Some(
                tree.iter()
                    .position(|info| Some(info.tree) == old_tree)
                    .unwrap_or(len - 1),
            )
        };

        self.tree = tree;
        self.update_table();
        self.table.widget.select(selected);
    }

    fn table_row(&self, info: TreeIterItem<'_, SnapshotNode, usize>) -> Vec<Text<'static>> {
        let (has_mark, has_not_mark, has_modified, has_to_forget) = info
            .tree
            .iter()
            .filter_map(|item| item.leaf_data().copied())
            .fold(
                (false, false, false, false),
                |(mut a, mut b, mut c, mut d), i| {
                    if self.snaps_status[i].marked {
                        a = true;
                    } else {
                        b = true;
                    }

                    if self.snaps_status[i].modified {
                        c = true;
                    }

                    if self.snaps_status[i].to_forget {
                        d = true;
                    }

                    (a, b, c, d)
                },
            );

        let mark = match (has_mark, has_not_mark) {
            (false, _) => " ",
            (true, true) => "*",
            (true, false) => "X",
        };
        let modified = if has_modified { "*" } else { " " };
        let del = if has_to_forget { "🗑" } else { "" };
        let mut collapse = "  ".repeat(info.depth);
        collapse.push_str(match info.tree {
            Tree::Leaf(_) => "",
            Tree::Node(TreeNode { open: false, .. }) => "\u{25b6} ", // Arrow to right
            Tree::Node(TreeNode { open: true, .. }) => "\u{25bc} ",  // Arrow down
        });

        match info.tree {
            Tree::Leaf(index)
            | Tree::Node(TreeNode {
                data: SnapshotNode::Snap(index),
                ..
            }) => {
                let snap = &self.snapshots[*index];
                let symbols = match (
                    snap.delete == DeleteOption::NotSet,
                    snap.description.is_none(),
                ) {
                    (true, true) => "",
                    (true, false) => "🗎",
                    (false, true) => "🛡",
                    (false, false) => "🛡 🗎",
                };
                let count = info.tree.child_count();
                once(&mark.to_string())
                    .chain(snap_to_table(snap, count).iter())
                    .cloned()
                    .enumerate()
                    .map(|(i, mut content)| {
                        if i == 1 {
                            // ID gets modified and protected marks
                            content = format!("{collapse}{modified}{del}{content}{symbols}");
                        }
                        Text::from(content)
                    })
                    .collect()
            }
            Tree::Node(TreeNode {
                data: SnapshotNode::Group(group),
                ..
            }) => {
                let host = group
                    .hostname
                    .as_ref()
                    .map(String::from)
                    .unwrap_or_default();
                let label = group.label.as_ref().map(String::from).unwrap_or_default();

                let paths = group
                    .paths
                    .as_ref()
                    .map_or_else(String::default, StringList::formatln);
                let tags = group
                    .tags
                    .as_ref()
                    .map_or_else(String::default, StringList::formatln);
                [
                    mark.to_string(),
                    format!("{collapse}{modified}{del}group"),
                    String::default(),
                    host,
                    label,
                    tags,
                    paths,
                    String::default(),
                    String::default(),
                    String::default(),
                ]
                .into_iter()
                .map(Text::from)
                .collect()
            }
        }
    }

    pub fn update_table(&mut self) {
        let max_tags = self
            .filtered_snapshots
            .iter()
            .map(|&i| self.snapshots[i].tags.iter().count())
            .max()
            .unwrap_or(1);
        let max_paths = self
            .filtered_snapshots
            .iter()
            .map(|&i| self.snapshots[i].paths.iter().count())
            .max()
            .unwrap_or(1);
        let height = max_tags.max(max_paths).max(1) + 1;

        let rows = self
            .tree
            .iter_open()
            .map(|tree| self.table_row(tree))
            .collect();

        self.table.widget.set_content(rows, height);
        self.table.block = Block::new()
            .borders(Borders::BOTTOM)
            .title_bottom(format!(
                "{:?} view: {}, total: {}, marked: {}, modified: {}, to forget: {}",
                self.current_view,
                self.filtered_snapshots.len(),
                self.snapshots.len(),
                self.count_marked_snaps(),
                self.count_modified_snaps(),
                self.count_forget_snaps()
            ))
            .title_alignment(Alignment::Center);
    }

    pub fn toggle_mark(&mut self) {
        for snap_idx in self.snap_idx() {
            self.snaps_status[snap_idx].toggle_mark();
        }
        self.update_table();
    }

    pub fn toggle_mark_all(&mut self) {
        for snap_idx in &self.filtered_snapshots {
            self.snaps_status[*snap_idx].toggle_mark();
        }
        self.update_table();
    }

    pub fn clear_marks(&mut self) {
        for status in &mut self.snaps_status {
            status.marked = false;
        }
        self.update_table();
    }

    pub fn reset_filter(&mut self) {
        self.filter = self.default_filter.clone();
        self.apply_view();
    }

    pub fn collapse(&mut self) {
        if let Some(tree) = self.selected_tree_mut() {
            tree.close();
            self.update_table();
        }
    }

    pub fn extendable(&self) -> bool {
        matches!(self.selected_tree(), Some(tree_info) if tree_info.tree.openable())
    }

    pub fn extend(&mut self) {
        if let Some(tree) = self.selected_tree_mut() {
            tree.open();
            self.update_table();
        }
    }

    pub fn snapshot_details(&self) -> PopUpTable {
        let mut rows = Vec::new();
        if let Some(snap) = self.selected_snapshot() {
            fill_table(snap, |title, value| {
                rows.push(vec![Text::from(title.to_string()), Text::from(value)]);
            });
        }
        popup_table("snapshot details", rows)
    }

    pub fn dir(&self) -> Result<Option<Snapshot<'a, P, S>>> {
        self.selected_snapshot().map_or(Ok(None), |snap| {
            Some(Snapshot::new(self.repo, snap.clone())).transpose()
        })
    }

    pub fn diff(&self) -> Result<Option<Diff<'a, P, S>>> {
        if self.count_marked_snaps() != 2 {
            return Ok(None);
        }

        let snaps: Vec<_> = self
            .snapshots
            .iter()
            .zip(self.snaps_status.iter())
            .filter_map(|(snap, status)| status.marked.then_some(snap))
            .collect();
        let left = snaps[0].tree;
        let right = snaps[1].tree;
        Some(Diff::new(self.repo, left, right)).transpose()
    }

    pub fn count_marked_snaps(&self) -> usize {
        self.snaps_status.iter().filter(|s| s.marked).count()
    }

    pub fn count_modified_snaps(&self) -> usize {
        self.snaps_status.iter().filter(|s| s.modified).count()
    }

    pub fn count_forget_snaps(&self) -> usize {
        self.snaps_status.iter().filter(|s| s.to_forget).count()
    }

    // process marked snapshots (or the current one if none is marked)
    // the process function must return true if it modified the snapshot, else false
    pub fn process_marked_snaps(&mut self, mut process: impl FnMut(&mut SnapshotFile) -> bool) {
        let has_mark = self.has_mark();

        if !has_mark {
            self.toggle_mark();
        }

        for ((snap, status), original_snap) in self
            .snapshots
            .iter_mut()
            .zip(self.snaps_status.iter_mut())
            .zip(self.original_snapshots.iter())
        {
            if status.marked && process(snap) {
                // Note that snap impls Eq, but only by comparing the time!
                status.modified =
                    serde_json::to_string(snap).ok() != serde_json::to_string(original_snap).ok();
            }
        }

        if !has_mark {
            self.toggle_mark();
        }
        self.update_table();
    }

    pub fn get_snap_entity(&mut self, f: impl Fn(&SnapshotFile) -> String) -> String {
        let has_mark = self.has_mark();

        if !has_mark {
            self.toggle_mark();
        }

        let entity = self
            .snapshots
            .iter()
            .zip(self.snaps_status.iter())
            .filter_map(|(snap, status)| status.marked.then_some(f(snap)))
            .reduce(|entity, e| if entity == e { e } else { String::new() })
            .unwrap_or_default();

        if !has_mark {
            self.toggle_mark();
        }
        entity
    }

    pub fn get_label(&mut self) -> String {
        self.get_snap_entity(|snap| snap.label.clone())
    }

    pub fn get_tags(&mut self) -> String {
        self.get_snap_entity(|snap| snap.tags.formatln())
    }

    pub fn get_description(&mut self) -> String {
        self.get_snap_entity(|snap| snap.description.clone().unwrap_or_default())
    }

    pub fn get_filter(&self) -> Result<String> {
        Ok(toml::to_string_pretty(&self.filter)?)
    }

    pub fn set_filter(&mut self, filter: String) {
        if let Ok(filter) = toml::from_str::<SnapshotFilter>(&filter) {
            self.filter = filter;
            self.apply_view();
        }
    }

    pub fn set_label(&mut self, label: String) {
        self.process_marked_snaps(|snap| {
            if snap.label == label {
                return false;
            }
            snap.label.clone_from(&label);
            true
        });
    }

    pub fn clear_label(&mut self) {
        self.set_label(String::new());
    }

    pub fn set_description(&mut self, desc: String) {
        let desc = if desc.is_empty() { None } else { Some(desc) };
        self.process_marked_snaps(|snap| {
            if snap.description == desc {
                return false;
            }
            snap.description.clone_from(&desc);
            true
        });
    }

    pub fn clear_description(&mut self) {
        self.set_description(String::new());
    }

    pub fn add_tags(&mut self, tags: String) {
        let tags = vec![StringList::from_str(&tags).unwrap()];
        self.process_marked_snaps(|snap| snap.add_tags(tags.clone()));
    }

    pub fn set_tags(&mut self, tags: String) {
        let tags = vec![StringList::from_str(&tags).unwrap()];
        self.process_marked_snaps(|snap| snap.set_tags(tags.clone()));
    }

    pub fn remove_tags(&mut self, tags: String) {
        let tags = vec![StringList::from_str(&tags).unwrap()];
        self.process_marked_snaps(|snap| snap.remove_tags(&tags));
    }

    pub fn clear_tags(&mut self) {
        let no_tags = vec![StringList::default()];
        self.process_marked_snaps(|snap| snap.set_tags(no_tags.clone()));
    }

    pub fn set_delete_protection_to(&mut self, delete: DeleteOption) {
        self.process_marked_snaps(|snap| {
            if snap.delete == delete {
                return false;
            }
            snap.delete = delete;
            true
        });
    }

    pub fn toggle_to_forget(&mut self) {
        let has_mark = self.has_mark();

        if !has_mark {
            self.toggle_mark();
        }

        let now = Local::now();
        for (snap, status) in self.snapshots.iter_mut().zip(self.snaps_status.iter_mut()) {
            if status.marked {
                if status.to_forget {
                    status.to_forget = false;
                } else if !snap.must_keep(now) {
                    status.to_forget = true;
                }
            }
        }

        if !has_mark {
            self.toggle_mark();
        }
        self.update_table();
    }

    pub fn clear_to_forget(&mut self) {
        for status in &mut self.snaps_status {
            status.to_forget = false;
        }
        self.update_table();
    }

    pub fn apply_input(&mut self, input: String) {
        match self.current_screen {
            CurrentScreen::EnterLabel(_) => self.set_label(input),
            CurrentScreen::EnterDescription(_) => self.set_description(input),
            CurrentScreen::EnterAddTags(_) => self.add_tags(input),
            CurrentScreen::EnterSetTags(_) => self.set_tags(input),
            CurrentScreen::EnterRemoveTags(_) => self.remove_tags(input),
            CurrentScreen::EnterFilter(_) => self.set_filter(input),
            _ => {}
        }
    }

    pub fn set_delete_protection(&mut self) {
        self.set_delete_protection_to(DeleteOption::Never);
    }

    pub fn clear_delete_protection(&mut self) {
        self.set_delete_protection_to(DeleteOption::NotSet);
    }

    pub fn write(&mut self) -> Result<()> {
        if !self.has_modified() && self.count_forget_snaps() == 0 {
            return Ok(());
        };

        let save_snaps: Vec<_> = self
            .snapshots
            .iter()
            .zip(self.snaps_status.iter())
            .filter_map(|(snap, status)| (status.modified && !status.to_forget).then_some(snap))
            .cloned()
            .collect();
        let old_snap_ids = save_snaps.iter().map(|sn| sn.id);
        let snap_ids_to_forget = self
            .snapshots
            .iter()
            .zip(self.snaps_status.iter())
            .filter_map(|(snap, status)| status.to_forget.then_some(snap.id));
        let delete_ids: Vec<_> = old_snap_ids.chain(snap_ids_to_forget).collect();
        self.repo.save_snapshots(save_snaps)?;
        self.repo.delete_snapshots(&delete_ids)?;
        // remove snapshots-to-reread
        let ids: BTreeSet<_> = delete_ids.into_iter().collect();
        self.snapshots.retain(|snap| !ids.contains(&snap.id));
        // re-read snapshots
        self.reread()
    }

    // re-read all snapshots
    pub fn reread(&mut self) -> Result<()> {
        let snapshots = mem::take(&mut self.snapshots);
        self.snapshots = self.repo.update_all_snapshots(snapshots)?;
        self.snapshots
            .sort_unstable_by(|sn1, sn2| sn1.cmp_group(self.group_by, sn2).then(sn1.cmp(sn2)));
        self.snaps_status = vec![SnapStatus::default(); self.snapshots.len()];
        self.original_snapshots.clone_from(&self.snapshots);
        self.table.widget.select(None);
        self.apply_view();
        Ok(())
    }

    pub fn input(&mut self, event: Event) -> Result<bool> {
        use KeyCode::{Char, Enter, Esc, F, Left, Right};
        match &mut self.current_screen {
            CurrentScreen::Snapshots => {
                match event {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        if key.modifiers == KeyModifiers::CONTROL {
                            match key.code {
                                Char('f') => self.clear_to_forget(),
                                Char('x') => self.clear_marks(),
                                Char('l') => self.clear_label(),
                                Char('d') => self.clear_description(),
                                Char('t') => self.clear_tags(),
                                Char('p') => self.clear_delete_protection(),
                                Char('v') => self.reset_filter(),
                                _ => {}
                            }
                        } else {
                            match key.code {
                                Esc | Char('q') => {
                                    self.current_screen = CurrentScreen::PromptExit(popup_prompt(
                                        "exit rustic",
                                        "do you want to exit? (y/n)".into(),
                                    ));
                                }
                                Char('f') => self.toggle_to_forget(),
                                F(5) => self.reread()?,
                                Enter => {
                                    if let Some(dir) = self.dir()? {
                                        self.current_screen = CurrentScreen::Dir(Box::new(dir));
                                    }
                                }
                                Right => {
                                    if self.extendable() {
                                        self.extend();
                                    } else if let Some(dir) = self.dir()? {
                                        self.current_screen = CurrentScreen::Dir(Box::new(dir));
                                    }
                                }
                                Char('+') => {
                                    if self.extendable() {
                                        self.extend();
                                    }
                                }
                                Left | Char('-') => self.collapse(),
                                Char('?') => {
                                    self.current_screen = CurrentScreen::ShowHelp(popup_text(
                                        "help",
                                        HELP_TEXT.into(),
                                    ));
                                }
                                Char('x') => {
                                    self.toggle_mark();
                                    self.table.widget.next();
                                }
                                Char('X') => self.toggle_mark_all(),
                                Char('v') => self.toggle_view(),
                                Char('V') => {
                                    self.current_screen = CurrentScreen::EnterFilter(popup_input(
                                        "set filter (Ctrl-s to confirm)",
                                        "enter filter in TOML format",
                                        &self.get_filter()?,
                                        15,
                                    ));
                                }
                                Char('i') => {
                                    self.current_screen =
                                        CurrentScreen::SnapshotDetails(self.snapshot_details());
                                }
                                Char('l') => {
                                    self.current_screen = CurrentScreen::EnterLabel(popup_input(
                                        "set label",
                                        "enter label",
                                        &self.get_label(),
                                        1,
                                    ));
                                }
                                Char('d') => {
                                    self.current_screen =
                                        CurrentScreen::EnterDescription(popup_input(
                                            "set description (Ctrl-s to confirm)",
                                            "enter description",
                                            &self.get_description(),
                                            5,
                                        ));
                                }
                                Char('D') => {
                                    if let Some(diff) = self.diff()? {
                                        self.current_screen = CurrentScreen::Diff(Box::new(diff));
                                    }
                                }
                                Char('t') => {
                                    self.current_screen = CurrentScreen::EnterAddTags(popup_input(
                                        "add tags",
                                        "enter tags",
                                        "",
                                        1,
                                    ));
                                }
                                Char('s') => {
                                    self.current_screen = CurrentScreen::EnterSetTags(popup_input(
                                        "set tags",
                                        "enter tags",
                                        &self.get_tags(),
                                        1,
                                    ));
                                }
                                Char('r') => {
                                    self.current_screen = CurrentScreen::EnterRemoveTags(
                                        popup_input("remove tags", "enter tags", "", 1),
                                    );
                                }
                                // TODO: Allow to enter delete protection option
                                Char('p') => self.set_delete_protection(),
                                Char('w') => {
                                    let msg = format!(
                                        "Do you want to write {} modified and remove {} snapshots? (y/n)",
                                        self.count_modified_snaps(),
                                        self.count_forget_snaps()
                                    );
                                    self.current_screen = CurrentScreen::PromptWrite(popup_prompt(
                                        "write snapshots",
                                        msg.into(),
                                    ));
                                }
                                _ => self.table.input(event),
                            }
                        }
                    }
                    _ => {}
                }
            }
            CurrentScreen::SnapshotDetails(_) | CurrentScreen::ShowHelp(_) => match event {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if matches!(key.code, Char('q' | ' ' | 'i' | '?') | Esc | Enter) {
                        self.current_screen = CurrentScreen::Snapshots;
                    }
                }
                _ => {}
            },
            CurrentScreen::EnterLabel(prompt)
            | CurrentScreen::EnterDescription(prompt)
            | CurrentScreen::EnterAddTags(prompt)
            | CurrentScreen::EnterSetTags(prompt)
            | CurrentScreen::EnterRemoveTags(prompt)
            | CurrentScreen::EnterFilter(prompt) => match prompt.input(event) {
                TextInputResult::Cancel => self.current_screen = CurrentScreen::Snapshots,
                TextInputResult::Input(input) => {
                    self.apply_input(input);
                    self.current_screen = CurrentScreen::Snapshots;
                }
                TextInputResult::None => {}
            },
            CurrentScreen::PromptWrite(prompt) => match prompt.input(event) {
                PromptResult::Ok => {
                    self.write()?;
                    self.current_screen = CurrentScreen::Snapshots;
                }
                PromptResult::Cancel => self.current_screen = CurrentScreen::Snapshots,
                PromptResult::None => {}
            },
            CurrentScreen::PromptExit(prompt) => match prompt.input(event) {
                PromptResult::Ok => return Ok(true),
                PromptResult::Cancel => self.current_screen = CurrentScreen::Snapshots,
                PromptResult::None => {}
            },
            CurrentScreen::Dir(dir) => match dir.input(event)? {
                SnapshotResult::Exit => return Ok(true),
                SnapshotResult::Return => self.current_screen = CurrentScreen::Snapshots,
                SnapshotResult::None => {}
            },
            CurrentScreen::Diff(diff) => match diff.input(event)? {
                DiffResult::Exit => return Ok(true),
                DiffResult::Return => self.current_screen = CurrentScreen::Snapshots,
                DiffResult::None => {}
            },
        }
        Ok(false)
    }

    pub fn draw(&mut self, area: Rect, f: &mut Frame<'_>) {
        if let CurrentScreen::Dir(dir) = &mut self.current_screen {
            dir.draw(area, f);
            return;
        }

        if let CurrentScreen::Diff(diff) = &mut self.current_screen {
            diff.draw(area, f);
            return;
        }

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
            CurrentScreen::SnapshotDetails(popup) => popup.draw(area, f),
            CurrentScreen::ShowHelp(popup) => popup.draw(area, f),
            CurrentScreen::EnterLabel(popup)
            | CurrentScreen::EnterDescription(popup)
            | CurrentScreen::EnterAddTags(popup)
            | CurrentScreen::EnterSetTags(popup)
            | CurrentScreen::EnterRemoveTags(popup)
            | CurrentScreen::EnterFilter(popup) => popup.draw(area, f),
            CurrentScreen::PromptWrite(popup) | CurrentScreen::PromptExit(popup) => {
                popup.draw(area, f);
            }
            _ => {}
        }
    }
}
