use std::{iter::once, str::FromStr};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{prelude::*, widgets::*};
use rustic_core::{
    repofile::{DeleteOption, SnapshotFile},
    OpenStatus, Repository, StringList,
};
use style::palette::tailwind;

use crate::{
    commands::{
        snapshots::{fill_table, snap_to_table},
        tui::widgets::{
            Draw, PopUp, ProcessEvent, Prompt, PromptResult, SelectTable, SizedParagraph,
            SizedTable, TextInput, TextInputResult, WithBlock,
        },
    },
    config::progress_options::ProgressOptions,
    filtering::SnapshotFilter,
};

// the widgets we are using and convenience builders
type PopUpInput = PopUp<WithBlock<TextInput>>;
fn popup_input(title: &'static str, text: &str, initial: &str) -> PopUpInput {
    PopUp(WithBlock::new(
        TextInput::new(text, initial),
        Block::bordered().title(title),
    ))
}

type PopUpText = PopUp<WithBlock<SizedParagraph>>;
fn popup_text(title: &'static str, text: Text<'static>) -> PopUpText {
    PopUp(WithBlock::new(
        SizedParagraph::new(text),
        Block::bordered().title(title),
    ))
}

type PopUpTable = PopUp<WithBlock<SizedTable>>;
fn popup_table(title: &'static str, content: Vec<Vec<Text<'static>>>) -> PopUpTable {
    PopUp(WithBlock::new(
        SizedTable::new(content),
        Block::bordered().title(title),
    ))
}

type PopUpPrompt = Prompt<PopUpText>;
fn popup_prompt(title: &'static str, text: Text<'static>) -> PopUpPrompt {
    Prompt(popup_text(title, text))
}

// the states this screen can be in
enum CurrentScreen {
    Snapshots,
    ShowHelp(PopUpText),
    SnapshotDetails(PopUpTable),
    EnterLabel(PopUpInput),
    EnterAddTags(PopUpInput),
    EnterSetTags(PopUpInput),
    EnterRemoveTags(PopUpInput),
    PromptWrite(PopUpPrompt),
}

#[derive(Clone, Copy, Default)]
struct SnapStatus {
    marked: bool,
    modified: bool,
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

#[derive(Clone, Copy)]
struct CollapserItem {
    child_count: usize,
    collapsed: bool,
}

struct CollapseInfo {
    index: usize,
    item: CollapserItem,
}

impl CollapseInfo {
    fn indices(&self) -> impl Iterator<Item = usize> {
        self.index..=(self.index + self.item.child_count)
    }
    fn collapsable(&self) -> bool {
        !self.item.collapsed
    }
    fn extendable(&self) -> bool {
        self.item.collapsed && self.item.child_count > 0
    }
}

struct Collapser(Vec<CollapserItem>);
struct CollapserIter<'a> {
    index: usize,
    showed_extended: bool,
    c: &'a [CollapserItem],
}

impl CollapserIter<'_> {
    fn increase_index(&mut self, inc: usize) {
        self.index += inc;

        // for l in self.level_open.iter_mut() {
        //     *l -= inc;
        // }
        // while self.level_open.last() == Some(&0) {
        //     _ = self.level_open.pop();
        // }
    }
}

impl Iterator for CollapserIter<'_> {
    type Item = CollapseInfo;

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.index;

        let mut item = *match self.c.get(index) {
            None => return None,
            Some(item) => item,
        };

        if item.collapsed {
            self.increase_index(item.child_count + 1);
        } else if !self.showed_extended {
            self.showed_extended = true;
        } else {
            self.increase_index(1);
            self.showed_extended = false;
            item.collapsed = false;
            item.child_count = 0;
        }

        Some(CollapseInfo { index, item })
    }
}

impl Collapser {
    fn iter(&self) -> CollapserIter<'_> {
        CollapserIter {
            index: 0,
            showed_extended: false,
            c: &self.0,
        }
    }

    fn collapse(&mut self, i: usize) {
        self.0[i].collapsed = true;
    }

    fn extend(&mut self, i: usize) {
        if self.0[i].child_count > 0 {
            self.0[i].collapsed = false;
        }
    }
}

pub(crate) struct App {
    current_screen: CurrentScreen,
    current_view: View,
    table: WithBlock<SelectTable>,
    repo: Repository<ProgressOptions, OpenStatus>,
    snaps_status: Vec<SnapStatus>,
    snapshots: Vec<SnapshotFile>,
    original_snapshots: Vec<SnapshotFile>,
    snaps_selection: Vec<usize>,
    snaps_collapse: Collapser, //position in snaps_selection and count
    filter: SnapshotFilter,
    default_filter: SnapshotFilter,
}

impl App {
    pub fn new(
        repo: Repository<ProgressOptions, OpenStatus>,
        filter: SnapshotFilter,
    ) -> Result<Self> {
        let mut snapshots = repo.get_all_snapshots()?;
        snapshots.sort_unstable();

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
            snaps_status: vec![SnapStatus::default(); snapshots.len()],
            original_snapshots: snapshots.clone(),
            snapshots,
            snaps_selection: Vec::new(),
            snaps_collapse: Collapser(Vec::new()),
            default_filter: filter.clone(),
            filter,
        };
        app.apply_filter();
        Ok(app)
    }

    fn selected_collapse_info(&self) -> Option<CollapseInfo> {
        self.table
            .widget
            .selected()
            .and_then(|selected| self.snaps_collapse.iter().nth(selected))
    }

    fn snap_idx(&self) -> Vec<usize> {
        self.selected_collapse_info()
            .iter()
            .flat_map(CollapseInfo::indices)
            .collect()
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
        self.apply_filter();
    }

    pub fn apply_filter(&mut self) {
        // remember current snapshot index
        let snap_id = self
            .snap_idx()
            .first()
            .map(|i| self.snapshots[self.snaps_selection[*i]].id);
        // select snapshots to show
        self.snaps_selection = self
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
        let len = self.snaps_selection.len();

        // collapse snapshots with identical treeid
        // we reverse iter snaps_selection as we need to count identical ids
        let mut id = None;
        let mut collapse = Vec::new();
        let mut same_tree: Vec<CollapserItem> = Vec::new();
        for i in &self.snaps_selection {
            let tree_id = self.snapshots[*i].tree;
            if id.is_some_and(|id| tree_id != id) {
                same_tree[0].child_count = same_tree.len() - 1;
                collapse.append(&mut same_tree);
            }
            id = Some(tree_id);
            same_tree.push(CollapserItem {
                child_count: 0,
                collapsed: true,
            });
        }
        same_tree[0].child_count = same_tree.len() - 1;
        collapse.append(&mut same_tree);
        self.snaps_collapse = Collapser(collapse);

        self.update_table();

        if len != 0 {
            let selected = self
                .snaps_collapse
                .iter()
                .position(|info| {
                    Some(self.snapshots[self.snaps_selection[info.index]].id) == snap_id
                })
                .unwrap_or(len - 1);

            self.table.widget.set_to(selected);
        }
    }

    fn snap_row(&self, info: CollapseInfo) -> Vec<Text<'static>> {
        let idx = info.index;
        let snap_id = self.snaps_selection[idx];
        let snap = &self.snapshots[snap_id];
        let symbols = match (
            snap.delete == DeleteOption::NotSet,
            snap.description.is_none(),
        ) {
            (true, true) => "",
            (true, false) => "🗎",
            (false, true) => "🛡",
            (false, false) => "🛡 🗎",
        };
        let mark = if info
            .indices()
            .all(|i| self.snaps_status[self.snaps_selection[i]].marked)
        {
            "X"
        } else if info
            .indices()
            .all(|i| !self.snaps_status[self.snaps_selection[i]].marked)
        {
            " "
        } else {
            "*"
        };
        let modified = if info
            .indices()
            .any(|i| self.snaps_status[self.snaps_selection[i]].modified)
        {
            "*"
        } else {
            " "
        };
        let count = info.item.child_count;
        let collapse = match (info.item.collapsed, info.item.child_count) {
            (_, 0) => "",
            (true, _) => ">",
            (false, _) => "v",
        };
        once(&mark.to_string())
            .chain(snap_to_table(snap, count).iter())
            .cloned()
            .enumerate()
            .map(|(i, mut content)| {
                if i == 1 {
                    // ID gets modified and protected marks
                    content = format!("{collapse}{modified}{content}{symbols}");
                }
                Text::from(content)
            })
            .collect()
    }

    pub fn update_table(&mut self) {
        let max_tags = self
            .snaps_selection
            .iter()
            .map(|&i| self.snapshots[i].tags.iter().count())
            .max()
            .unwrap_or(1);
        let max_paths = self
            .snaps_selection
            .iter()
            .map(|&i| self.snapshots[i].paths.iter().count())
            .max()
            .unwrap_or(1);
        let height = max_tags.max(max_paths).max(1) + 1;

        let mut rows = Vec::new();
        for collapse_info in self.snaps_collapse.iter() {
            let row = self.snap_row(collapse_info);
            rows.push(row);
        }

        self.table.widget.set_content(rows, height);
        self.table.block = Block::new()
            .borders(Borders::BOTTOM)
            .title_bottom(format!(
                "{:?} view: {}, total: {}, marked: {}, modified: {}, ",
                self.current_view,
                self.snaps_selection.len(),
                self.snapshots.len(),
                self.count_marked_snaps(),
                self.count_modified_snaps(),
            ))
            .title_alignment(Alignment::Center);
    }

    pub fn toggle_mark(&mut self) {
        for snap_idx in self.snap_idx() {
            self.snaps_status[self.snaps_selection[snap_idx]].toggle_mark();
        }
        self.update_table();
    }

    pub fn toggle_mark_all(&mut self) {
        for snap_idx in &self.snaps_selection {
            self.snaps_status[*snap_idx].toggle_mark();
        }
        self.update_table();
    }

    pub fn clear_marks(&mut self) {
        for status in self.snaps_status.iter_mut() {
            status.marked = false;
        }
        self.update_table();
    }

    pub fn clear_filter(&mut self) {
        self.filter = SnapshotFilter::default();
        self.apply_filter();
    }

    pub fn reset_filter(&mut self) {
        self.filter = self.default_filter.clone();
        self.apply_filter();
    }

    pub fn collapse(&mut self) {
        if let Some(info) = self.selected_collapse_info() {
            if info.collapsable() {
                self.snaps_collapse.collapse(info.index);
                self.update_table();
            }
        }
    }

    pub fn extend(&mut self) {
        if let Some(info) = self.selected_collapse_info() {
            if info.extendable() {
                self.snaps_collapse.extend(info.index);
                self.update_table();
            }
        }
    }

    pub fn snapshot_details(&self) -> PopUpTable {
        let mut rows = Vec::new();
        if let Some(info) = self.selected_collapse_info() {
            let snap = &self.snapshots[info.index];
            fill_table(snap, |title, value| {
                rows.push(vec![Text::from(title.to_string()), Text::from(value)]);
            });
        }
        popup_table("snapshot details", rows)
    }

    pub fn count_marked_snaps(&self) -> usize {
        self.snaps_status.iter().filter(|s| s.marked).count()
    }

    pub fn count_modified_snaps(&self) -> usize {
        self.snaps_status.iter().filter(|s| s.modified).count()
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
                status.modified = serde_json::to_string(snap).unwrap()
                    != serde_json::to_string(original_snap).unwrap();
            }
        }

        if !has_mark {
            self.toggle_mark();
        }
        self.apply_filter();
    }

    pub fn get_label(&mut self) -> String {
        let has_mark = self.has_mark();

        if !has_mark {
            self.toggle_mark();
        }

        let label = self
            .snapshots
            .iter()
            .zip(self.snaps_status.iter())
            .filter_map(|(snap, status)| status.marked.then_some(snap.label.clone()))
            .reduce(|label, l| if label == l { l } else { String::new() })
            .unwrap_or_default();

        if !has_mark {
            self.toggle_mark();
        }
        label
    }

    pub fn get_tags(&mut self) -> String {
        let has_mark = self.has_mark();

        if !has_mark {
            self.toggle_mark();
        }

        let label = self
            .snapshots
            .iter()
            .zip(self.snaps_status.iter())
            .filter_map(|(snap, status)| status.marked.then_some(snap.tags.formatln()))
            .reduce(|tags, t| if tags == t { t } else { String::new() })
            .unwrap_or_default();

        if !has_mark {
            self.toggle_mark();
        }
        label
    }

    pub fn set_label(&mut self, label: String) {
        self.process_marked_snaps(|snap| {
            if snap.label == label {
                return false;
            }
            snap.label = label.clone();
            true
        });
    }

    pub fn clear_label(&mut self) {
        self.set_label(String::new());
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

    pub fn set_delete_to(&mut self, delete: DeleteOption) {
        self.process_marked_snaps(|snap| {
            if snap.delete == delete {
                return false;
            }
            snap.delete = delete;
            true
        });
    }

    pub fn apply_input(&mut self, input: String) {
        match self.current_screen {
            CurrentScreen::EnterLabel(_) => self.set_label(input),
            CurrentScreen::EnterAddTags(_) => self.add_tags(input),
            CurrentScreen::EnterSetTags(_) => self.set_tags(input),
            CurrentScreen::EnterRemoveTags(_) => self.remove_tags(input),
            _ => {}
        }
    }

    pub fn set_delete_protection(&mut self) {
        self.set_delete_to(DeleteOption::Never);
    }

    pub fn clear_delete_protection(&mut self) {
        self.set_delete_to(DeleteOption::NotSet);
    }

    pub fn write(&mut self) -> Result<()> {
        if !self.has_modified() {
            return Ok(());
        };

        let save_snaps: Vec<_> = self
            .snapshots
            .iter()
            .zip(self.snaps_status.iter())
            .filter_map(|(snap, status)| status.modified.then_some(snap))
            .cloned()
            .collect();
        let old_snap_ids: Vec<_> = save_snaps.iter().map(|sn| sn.id).collect();
        self.repo.save_snapshots(save_snaps)?;
        self.repo.delete_snapshots(&old_snap_ids)?;
        // re-read snapshots
        self.reread()
    }

    // re-read all snapshots
    pub fn reread(&mut self) -> Result<()> {
        self.snapshots = self.repo.get_all_snapshots()?;
        self.snapshots.sort_unstable();
        for status in self.snaps_status.iter_mut() {
            status.modified = false;
        }
        self.original_snapshots = self.snapshots.clone();
        self.table.widget.select(None);
        self.apply_filter();
        Ok(())
    }
}

const INFO_TEXT: &str =
    "(Esc) quit | (F5) reload snaphots | (v) toggle view | (i) show snapshot | (?) show all commands";

const HELP_TEXT: &str = r#"
General Commands:

  q,Esc : exit
     F5 : re-read all snapshots from repository
      v : toggle snapshot view [Filtered -> All -> Marked -> Modified]
      i : show detailed snapshot information for selected snapshot
      w : write modified snapshots
      ? : show this help page

Commands for marking snapshot(s):

      x : toggle marking for selected snapshot
      X : toggle markings for all snapshots
 Ctrl-x : clear all markings

Commands applied to marked snapshot(s) (selected if none marked):

      l : set label for snapshot(s)
 Ctrl-l : remove label for snapshot(s)
      t : add tag(s) for snapshot(s)
 Ctrl-t : remove all tags for snapshot(s)
      s : set tag(s) for snapshot(s)
      r : remove tag(s) for snapshot(s)
      p : set delete protection for snapshot(s)
 Ctrl-p : remove delete protection for snapshot(s)
 "#;

pub(crate) fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    loop {
        _ = terminal.draw(|f| ui(f, &mut app))?;

        let event = event::read()?;
        use KeyCode::*;
        match &mut app.current_screen {
            CurrentScreen::Snapshots => {
                match event {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        if key.modifiers == KeyModifiers::CONTROL {
                            match key.code {
                                Char('x') => app.clear_marks(),
                                Char('f') => app.clear_filter(),
                                Char('l') => app.clear_label(),
                                Char('t') => app.clear_tags(),
                                Char('p') => app.clear_delete_protection(),
                                _ => {}
                            }
                        } else {
                            match key.code {
                                Char('q') | Esc => return Ok(()),
                                F(5) => app.reread()?,
                                Right => app.extend(),
                                Left => app.collapse(),
                                Char('?') => {
                                    app.current_screen = CurrentScreen::ShowHelp(popup_text(
                                        "help",
                                        HELP_TEXT.into(),
                                    ));
                                }
                                Char('x') => {
                                    app.toggle_mark();
                                    app.table.widget.next();
                                }
                                Char('X') => app.toggle_mark_all(),
                                Char('F') => app.reset_filter(),
                                Char('v') => app.toggle_view(),
                                Char('i') => {
                                    app.current_screen =
                                        CurrentScreen::SnapshotDetails(app.snapshot_details());
                                }
                                Char('l') => {
                                    app.current_screen = CurrentScreen::EnterLabel(popup_input(
                                        "set label",
                                        "enter label",
                                        &app.get_label(),
                                    ));
                                }
                                Char('t') => {
                                    app.current_screen = CurrentScreen::EnterAddTags(popup_input(
                                        "add tags",
                                        "enter tags",
                                        "",
                                    ));
                                }
                                Char('s') => {
                                    app.current_screen = CurrentScreen::EnterSetTags(popup_input(
                                        "set tags",
                                        "enter tags",
                                        &app.get_tags(),
                                    ));
                                }
                                Char('r') => {
                                    app.current_screen = CurrentScreen::EnterRemoveTags(
                                        popup_input("remove tags", "enter tags", ""),
                                    );
                                }
                                // TODO: Allow to enter delete protection option
                                Char('p') => app.set_delete_protection(),
                                Char('w') => {
                                    let msg = format!(
                                        "Do you want to write {} modified snapshots?",
                                        app.count_modified_snaps()
                                    );
                                    app.current_screen = CurrentScreen::PromptWrite(popup_prompt(
                                        "write snapshots",
                                        msg.into(),
                                    ));
                                }
                                _ => app.table.input(event),
                            }
                        }
                    }
                    _ => {}
                }
            }
            CurrentScreen::SnapshotDetails(_) | CurrentScreen::ShowHelp(_) => match event {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if matches!(
                        key.code,
                        Char('q') | Esc | Enter | Char(' ') | Char('i') | Char('?')
                    ) {
                        app.current_screen = CurrentScreen::Snapshots;
                    }
                }
                _ => {}
            },
            CurrentScreen::EnterLabel(prompt)
            | CurrentScreen::EnterAddTags(prompt)
            | CurrentScreen::EnterSetTags(prompt)
            | CurrentScreen::EnterRemoveTags(prompt) => match prompt.input(event) {
                TextInputResult::Cancel => app.current_screen = CurrentScreen::Snapshots,
                TextInputResult::Input(input) => {
                    app.apply_input(input);
                    app.current_screen = CurrentScreen::Snapshots;
                }
                TextInputResult::None => {}
            },
            CurrentScreen::PromptWrite(prompt) => match prompt.input(event) {
                PromptResult::Ok => {
                    app.write()?;
                    app.current_screen = CurrentScreen::Snapshots;
                }
                PromptResult::Cancel => app.current_screen = CurrentScreen::Snapshots,
                PromptResult::None => {}
            },
        }
    }
}

fn ui(f: &mut Frame<'_>, app: &mut App) {
    let area = f.size();
    let rects = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);

    // draw the table
    app.table.draw(rects[0], f);

    // draw the footer
    let buffer_bg = tailwind::SLATE.c950;
    let row_fg = tailwind::SLATE.c200;
    let info_footer = Paragraph::new(Line::from(INFO_TEXT))
        .style(Style::new().fg(row_fg).bg(buffer_bg))
        .centered();
    f.render_widget(info_footer, rects[1]);

    // draw popups
    match &mut app.current_screen {
        CurrentScreen::Snapshots => {}
        CurrentScreen::SnapshotDetails(popup) => popup.draw(area, f),
        CurrentScreen::ShowHelp(popup) => popup.draw(area, f),
        CurrentScreen::EnterLabel(popup)
        | CurrentScreen::EnterAddTags(popup)
        | CurrentScreen::EnterSetTags(popup)
        | CurrentScreen::EnterRemoveTags(popup) => popup.draw(area, f),
        CurrentScreen::PromptWrite(popup) => popup.draw(area, f),
    }
}
