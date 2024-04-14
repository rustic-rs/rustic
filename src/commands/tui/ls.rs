use std::path::PathBuf;

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::{prelude::*, widgets::*};
use rustic_core::{
    repofile::{Node, SnapshotFile, Tree},
    IndexedFull, Repository,
};
use style::palette::tailwind;

use crate::{
    commands::{
        ls::{NodeLs, Summary},
        tui::widgets::{popup_text, Draw, PopUpText, ProcessEvent, SelectTable, WithBlock},
    },
    config::progress_options::ProgressOptions,
};

// the states this screen can be in
enum CurrentScreen {
    Snapshot,
    ShowHelp(PopUpText),
}

const INFO_TEXT: &str =
    "(Esc) quit | (Enter) enter dir | (Backspace) return to parent | (?) show all commands";

const HELP_TEXT: &str = r#"
General Commands:

      q,Esc : exit
      Enter : enter dir
  Backspace : return to parent dir
          n : toggle numeric IDs
          ? : show this help page

 "#;

pub(crate) struct Snapshot<'a, S> {
    current_screen: CurrentScreen,
    numeric: bool,
    table: WithBlock<SelectTable>,
    repo: &'a Repository<ProgressOptions, S>,
    snapshot: SnapshotFile,
    path: PathBuf,
    trees: Vec<Tree>,
}

impl<'a, S: IndexedFull> Snapshot<'a, S> {
    pub fn new(repo: &'a Repository<ProgressOptions, S>, snapshot: SnapshotFile) -> Result<Self> {
        let header = ["Name", "Size", "Mode", "User", "Group", "Time"]
            .into_iter()
            .map(Text::from)
            .collect();

        let tree = repo.get_tree(&snapshot.tree)?;
        let mut app = Self {
            current_screen: CurrentScreen::Snapshot,
            numeric: false,
            table: WithBlock::new(SelectTable::new(header), Block::new()),
            repo,
            snapshot,
            path: PathBuf::new(),
            trees: vec![tree],
        };
        app.update_table();
        Ok(app)
    }

    fn ls_row(&self, node: &Node) -> Vec<Text<'static>> {
        let (user, group) = if self.numeric {
            (
                node.meta
                    .uid
                    .map_or_else(|| "?".to_string(), |id| id.to_string()),
                node.meta
                    .gid
                    .map_or_else(|| "?".to_string(), |id| id.to_string()),
            )
        } else {
            (
                node.meta.user.clone().unwrap_or_else(|| "?".to_string()),
                node.meta.group.clone().unwrap_or_else(|| "?".to_string()),
            )
        };
        let name = node.name().to_string_lossy().to_string();
        let size = node.meta.size.to_string();
        let mtime = node
            .meta
            .mtime
            .map(|t| format!("{}", t.format("%Y-%m-%d %H:%M:%S")))
            .unwrap_or_else(|| "?".to_string());
        [name, size, node.mode_str(), user, group, mtime]
            .into_iter()
            .map(Text::from)
            .collect()
    }

    pub fn update_table(&mut self) {
        let old_selection = self.table.widget.selected();
        let tree = self.trees.last().unwrap();
        let mut rows = Vec::new();
        let mut summary = Summary::default();
        for node in &tree.nodes {
            summary.update(node);
            let row = self.ls_row(node);
            rows.push(row);
        }

        self.table.widget.set_content(rows, 1);

        self.table.block = Block::new()
            .borders(Borders::BOTTOM | Borders::TOP)
            .title(format!("{}:{}", self.snapshot.id, self.path.display()))
            .title_bottom(format!(
                "total: {}, files: {}, dirs: {}, size: {} - {}",
                tree.nodes.len(),
                summary.files,
                summary.dirs,
                summary.size,
                if self.numeric {
                    "numeric IDs"
                } else {
                    " Id names"
                }
            ))
            .title_alignment(Alignment::Center);
        self.table.widget.set_to(old_selection.unwrap_or_default());
    }

    pub fn enter(&mut self) -> Result<()> {
        if let Some(idx) = self.table.widget.selected() {
            let node = &self.trees.last().unwrap().nodes[idx];
            if node.is_dir() {
                self.path.push(node.name());
                self.trees.push(self.repo.get_tree(&node.subtree.unwrap())?);
            }
        }
        self.update_table();
        Ok(())
    }

    pub fn goback(&mut self) -> bool {
        _ = self.path.pop();
        _ = self.trees.pop();
        if !self.trees.is_empty() {
            self.update_table();
        }
        self.trees.is_empty()
    }

    pub fn toggle_numeric(&mut self) {
        self.numeric = !self.numeric;
        self.update_table();
    }

    pub fn input(&mut self, event: Event) -> Result<bool> {
        use KeyCode::*;
        match &mut self.current_screen {
            CurrentScreen::Snapshot => match event {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    Enter | Right => self.enter()?,
                    Backspace | Left => {
                        if self.goback() {
                            return Ok(true);
                        }
                    }
                    Char('?') => {
                        self.current_screen =
                            CurrentScreen::ShowHelp(popup_text("help", HELP_TEXT.into()));
                    }
                    Char('n') => self.toggle_numeric(),
                    _ => self.table.input(event),
                },
                _ => {}
            },
            CurrentScreen::ShowHelp(_) => match event {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if matches!(key.code, Char('q') | Esc | Enter | Char(' ') | Char('?')) {
                        self.current_screen = CurrentScreen::Snapshot;
                    }
                }
                _ => {}
            },
        }
        Ok(false)
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
        }
    }
}
