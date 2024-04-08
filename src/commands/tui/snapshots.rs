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
        tui::widgets::{PopUpInputResult, PopUpPromptResult},
    },
    config::progress_options::ProgressOptions,
    filtering::SnapshotFilter,
};

use super::widgets::{PopUpInput, PopUpPrompt, PopUpTable};

const INFO_TEXT: &str =
    "(Esc) quit | (F5) reload snaphots | (v) toggle view | (i) show snapshot | (?) show all shortcuts";

struct TableColors {
    buffer_bg: Color,
    header_bg: Color,
    header_fg: Color,
    row_fg: Color,
    selected_style_fg: Color,
    normal_row_color: Color,
    alt_row_color: Color,
}

impl TableColors {
    fn new(color: &tailwind::Palette) -> Self {
        Self {
            buffer_bg: tailwind::SLATE.c950,
            header_bg: color.c900,
            header_fg: tailwind::SLATE.c200,
            row_fg: tailwind::SLATE.c200,
            selected_style_fg: color.c400,
            normal_row_color: tailwind::SLATE.c950,
            alt_row_color: tailwind::SLATE.c900,
        }
    }
}

enum CurrentScreen {
    Snapshots,
    ShowHelp(PopUpTable),
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

pub(crate) struct App {
    state: TableState,
    scroll_state: ScrollbarState,
    rows: usize,
    height: usize,
    current_screen: CurrentScreen,
    current_view: View,

    repo: Repository<ProgressOptions, OpenStatus>,
    snaps_status: Vec<SnapStatus>,
    snapshots: Vec<SnapshotFile>,
    original_snapshots: Vec<SnapshotFile>,
    snaps_selection: Vec<usize>,
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

        let mut app = Self {
            state: TableState::default(),
            scroll_state: ScrollbarState::new(0),
            rows: 5,
            height: 1,
            current_screen: CurrentScreen::Snapshots,
            current_view: View::Filter,

            repo,
            snaps_status: vec![SnapStatus::default(); snapshots.len()],
            original_snapshots: snapshots.clone(),
            snapshots,
            snaps_selection: Vec::new(),
            default_filter: filter.clone(),
            filter,
        };
        app.apply_filter();
        Ok(app)
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
        let snap_idx = self.state.selected().map(|i| self.snaps_selection[i]);
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
        self.height = max_tags.max(max_paths).max(1) + 1;

        self.scroll_state = ScrollbarState::new(len * self.height);

        if len == 0 {
            self.state = TableState::default();
        } else {
            // see if the current snapshot is still available and if, select it.
            let selected = self
                .snaps_selection
                .iter()
                .position(|&s| Some(s) == snap_idx)
                .unwrap_or(len - 1);

            self.state = TableState::default().with_selected(selected);
        }
    }

    pub fn set_to(&mut self, i: usize) {
        self.state.select(Some(i));
        self.scroll_state = self.scroll_state.position(i * self.height);
    }

    pub fn go_forward(&mut self, step: usize) {
        if let Some(selected_old) = self.state.selected() {
            let selected = (selected_old + step).min(self.snaps_selection.len() - 1);
            self.set_to(selected);
        }
    }

    pub fn go_back(&mut self, step: usize) {
        if let Some(selected_old) = self.state.selected() {
            let selected = selected_old.saturating_sub(step);
            self.set_to(selected);
        }
    }

    pub fn next(&mut self) {
        self.go_forward(1);
    }

    pub fn page_down(&mut self) {
        self.go_forward(self.rows);
    }

    pub fn previous(&mut self) {
        self.go_back(1);
    }

    pub fn page_up(&mut self) {
        self.go_back(self.rows);
    }

    pub fn home(&mut self) {
        if self.state.selected().is_some() {
            self.set_to(0);
        }
    }

    pub fn end(&mut self) {
        if self.state.selected().is_some() {
            self.set_to(self.snaps_selection.len() - 1);
        }
    }

    pub fn set_rows(&mut self, rows: usize) {
        self.rows = rows / self.height;
    }

    pub fn toggle_mark(&mut self) {
        if let Some(i) = self.state.selected() {
            let snap_idx = self.snaps_selection[i];
            self.snaps_status[snap_idx].toggle_mark();
        }
    }

    pub fn toggle_mark_all(&mut self) {
        for snap_idx in &self.snaps_selection {
            self.snaps_status[*snap_idx].toggle_mark();
        }
    }

    pub fn clear_marks(&mut self) {
        for status in self.snaps_status.iter_mut() {
            status.marked = false;
        }
    }

    pub fn clear_filter(&mut self) {
        self.filter = SnapshotFilter::default();
        self.apply_filter();
    }

    pub fn reset_filter(&mut self) {
        self.filter = self.default_filter.clone();
        self.apply_filter();
    }

    pub fn snapshot_details(&self) -> PopUpTable {
        let mut rows = Vec::new();
        if let Some(selected) = self.state.selected() {
            let snap = &self.snapshots[self.snaps_selection[selected]];
            fill_table(snap, |title, value| {
                rows.push(vec![Text::from(title.to_string()), Text::from(value)]);
            });
        }
        PopUpTable::new("snapshot details", rows)
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
        self.apply_filter();
    }

    pub fn set_tags(&mut self, tags: String) {
        let tags = vec![StringList::from_str(&tags).unwrap()];
        self.process_marked_snaps(|snap| snap.set_tags(tags.clone()));
        self.apply_filter();
    }

    pub fn remove_tags(&mut self, tags: String) {
        let tags = vec![StringList::from_str(&tags).unwrap()];
        self.process_marked_snaps(|snap| snap.remove_tags(&tags));
        self.apply_filter();
    }

    pub fn clear_tags(&mut self) {
        let no_tags = vec![StringList::default()];
        self.process_marked_snaps(|snap| snap.set_tags(no_tags.clone()));
        self.apply_filter();
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
        self.state.select(None);
        self.apply_filter();
        Ok(())
    }
}

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
                                Down => app.next(),
                                Up => app.previous(),
                                PageDown => app.page_down(),
                                PageUp => app.page_up(),
                                Home => app.home(),
                                End => app.end(),
                                F(5) => app.reread()?,
                                Char('?') => {
                                    let rows = vec![
                                        vec![Text::from("Esc | q"), Text::from("exit")],
                                        vec![Text::from("F5"), Text::from("re-read all snapshots from repository")],
                                        vec![Text::from("v"), Text::from("toggle snapshot view [Filtered -> All -> Marked -> Modified]")],
                                        vec![
                                            Text::from("x"),
                                            Text::from("toggle marking for selected snapshot"),
                                        ],
                                        vec![
                                            Text::from("X"),
                                            Text::from("toggle markings for all snapshots"),
                                        ],
                                        vec![
                                            Text::from("Ctrl-x"),
                                            Text::from("clear all markings"),
                                        ],
                                        vec![
                                            Text::from("i"),
                                            Text::from("show detailed snapshot information for selected snapshot"),
                                        ],
                                        vec![
                                            Text::from("l"),
                                            Text::from("set label for snapshot(s)"),
                                        ],
                                        vec![
                                            Text::from("Ctrl-l"),
                                            Text::from("remove label for snapshot(s)"),
                                        ],
                                        vec![
                                            Text::from("t"),
                                            Text::from("add tag(s) for snapshot(s)"),
                                        ],
                                        vec![
                                            Text::from("Ctrl-t"),
                                            Text::from("remove all tags for snapshot(s)"),
                                        ],
                                        vec![
                                            Text::from("s"),
                                            Text::from("set tag(s) for snapshot(s)"),
                                        ],
                                        vec![
                                            Text::from("r"),
                                            Text::from("remove tag(s) for snapshot(s)"),
                                        ],
                                        vec![
                                            Text::from("p"),
                                            Text::from("set delete protection for snapshot(s)"),
                                        ],
                                        vec![
                                            Text::from("Ctrl-p"),
                                            Text::from("remove delete protection for snapshot(s)"),
                                        ],
                                        vec![
                                            Text::from("w"),
                                            Text::from("write modified snapshots"),
                                        ],
                                    ];
                                    app.current_screen =
                                        CurrentScreen::ShowHelp(PopUpTable::new("help", rows));
                                }
                                Char('x') => {
                                    app.toggle_mark();
                                    app.next();
                                }
                                Char('X') => app.toggle_mark_all(),
                                Char('F') => app.reset_filter(),
                                Char('v') => app.toggle_view(),
                                Char('i') => {
                                    app.current_screen =
                                        CurrentScreen::SnapshotDetails(app.snapshot_details());
                                }
                                Char('l') => {
                                    app.current_screen =
                                        CurrentScreen::EnterLabel(PopUpInput::new(
                                            "enter label",
                                            "set label",
                                            &app.get_label(),
                                        ));
                                }
                                Char('t') => {
                                    app.current_screen = CurrentScreen::EnterAddTags(
                                        PopUpInput::new("enter tags", "add tags", ""),
                                    );
                                }
                                Char('s') => {
                                    app.current_screen = CurrentScreen::EnterSetTags(
                                        PopUpInput::new("enter tags", "set tags", &app.get_tags()),
                                    );
                                }
                                Char('r') => {
                                    app.current_screen = CurrentScreen::EnterRemoveTags(
                                        PopUpInput::new("enter tags", "remove tags", ""),
                                    );
                                }
                                // TODO: Allow to enter delete protection option
                                Char('p') => app.set_delete_protection(),
                                Char('w') => {
                                    let msg = format!(
                                        "Do you want to write {} modified snapshots?",
                                        app.count_modified_snaps()
                                    );
                                    app.current_screen = CurrentScreen::PromptWrite(
                                        PopUpPrompt::new("write snapshots", msg),
                                    );
                                }
                                _ => {}
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
                PopUpInputResult::Cancel => app.current_screen = CurrentScreen::Snapshots,
                PopUpInputResult::Input(input) => {
                    app.apply_input(input);
                    app.current_screen = CurrentScreen::Snapshots;
                }
                PopUpInputResult::None => {}
            },
            CurrentScreen::PromptWrite(prompt) => match prompt.input(event) {
                PopUpPromptResult::Ok => {
                    app.write()?;
                    app.current_screen = CurrentScreen::Snapshots;
                }
                PopUpPromptResult::Cancel => app.current_screen = CurrentScreen::Snapshots,
                PopUpPromptResult::None => {}
            },
        }
    }
}

fn ui(f: &mut Frame<'_>, app: &mut App) {
    let rects = Layout::vertical([Constraint::Min(5), Constraint::Length(1)]).split(f.size());

    app.set_rows(rects[0].height.into());
    let colors = TableColors::new(&tailwind::BLUE);

    render_table(f, app, rects[0], &colors);
    render_scrollbar(f, app, rects[0]);
    render_footer(f, rects[1], &colors);

    match &app.current_screen {
        CurrentScreen::Snapshots => {}
        CurrentScreen::SnapshotDetails(popup) | CurrentScreen::ShowHelp(popup) => popup.render(f),
        CurrentScreen::EnterLabel(popup)
        | CurrentScreen::EnterAddTags(popup)
        | CurrentScreen::EnterSetTags(popup)
        | CurrentScreen::EnterRemoveTags(popup) => popup.render(f),
        CurrentScreen::PromptWrite(popup) => popup.render(f),
    }
}

fn render_table(f: &mut Frame<'_>, app: &mut App, area: Rect, colors: &TableColors) {
    let header_style = Style::default().fg(colors.header_fg).bg(colors.header_bg);
    let selected_style = Style::default()
        .add_modifier(Modifier::REVERSED)
        .fg(colors.selected_style_fg);

    let header = Row::new(vec![
        "", " ID", "Time", "Host", "Label", "Tags", "Paths", "Files", "Dirs", "Size",
    ])
    .style(header_style);
    let rows = app.snaps_selection.iter().enumerate().map(|(i, &snap_id)| {
        let color = match i % 2 {
            0 => colors.normal_row_color,
            _ => colors.alt_row_color,
        };
        let status = app.snaps_status[snap_id];
        let snap = &app.snapshots[snap_id];
        let mark = if status.marked { "X" } else { " " };
        once(&mark.to_string())
            .chain(snap_to_table(snap, 0).iter())
            .cloned()
            .enumerate()
            .map(|(i, mut content)| {
                let modified = if status.modified { "*" } else { " " };
                let protected = if snap.delete == DeleteOption::NotSet {
                    ""
                } else {
                    "🛡"
                };
                let description = if snap.description.is_none() {
                    ""
                } else {
                    "🗎"
                };
                if i == 1 {
                    // ID gets modified and protected marks
                    content = format!("{modified}{content}{protected}{description}");
                }
                Cell::from(Text::from(content))
            })
            .collect::<Row<'_>>()
            .style(Style::new().fg(colors.row_fg).bg(color))
            .height(app.height.try_into().unwrap())
    });
    let t = Table::new(
        rows,
        [
            // + 1 is for padding.
            Constraint::Length(1 + 1),
            Constraint::Length(10 + 1),
            Constraint::Length(20 + 1),
            Constraint::Min(8 + 1),
            Constraint::Min(10 + 1),
            Constraint::Min(10 + 1),
            Constraint::Min(10 + 1),
            Constraint::Min(3 + 1),
            Constraint::Min(3 + 1),
            Constraint::Min(8),
        ],
    )
    .header(header)
    .highlight_style(selected_style)
    .bg(colors.buffer_bg)
    .block(Block::new().borders(Borders::BOTTOM).title_bottom(format!(
        "total: {}, marked: {}, modified: {}, view: {:?}",
        app.snaps_selection.len(),
        app.count_marked_snaps(),
        app.count_modified_snaps(),
        app.current_view,
    )));
    f.render_stateful_widget(t, area, &mut app.state);
}

fn render_scrollbar(f: &mut Frame<'_>, app: &mut App, area: Rect) {
    f.render_stateful_widget(
        Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None),
        area.inner(&Margin {
            vertical: 1,
            horizontal: 1,
        }),
        &mut app.scroll_state,
    );
}

fn render_footer(f: &mut Frame<'_>, area: Rect, colors: &TableColors) {
    let info_footer = Paragraph::new(Line::from(INFO_TEXT))
        .style(Style::new().fg(colors.row_fg).bg(colors.buffer_bg))
        .centered();
    f.render_widget(info_footer, area);
}
