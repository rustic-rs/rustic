use std::path::PathBuf;

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::{prelude::*, widgets::*};
use rustic_core::{
    repofile::{Node, SnapshotFile, Tree},
    IndexedFull, ProgressBars, Repository,
};
use style::palette::tailwind;

use crate::commands::{
    ls::{NodeLs, Summary},
    tui::{
        restore::Restore,
        widgets::{popup_text, Draw, PopUpText, ProcessEvent, SelectTable, WithBlock},
    },
};

// the states this screen can be in
enum CurrentScreen<'a, P, S> {
    Snapshot,
    ShowHelp(PopUpText),
    Restore(Restore<'a, P, S>),
}

const INFO_TEXT: &str =
    "(Esc) quit | (Enter) enter dir | (Backspace) return to parent | (r) restore | (?) show all commands";

const HELP_TEXT: &str = r#"
General Commands:

      q,Esc : exit
      Enter : enter dir
  Backspace : return to parent dir
          r : restore selected item
          n : toggle numeric IDs
          ? : show this help page

 "#;

pub(crate) struct Snapshot<'a, P, S> {
    current_screen: CurrentScreen<'a, P, S>,
    numeric: bool,
    table: WithBlock<SelectTable>,
    repo: &'a Repository<P, S>,
    snapshot: SnapshotFile,
    path: PathBuf,
    trees: Vec<Tree>,
    tree: Tree,
}

impl<'a, P: ProgressBars, S: IndexedFull> Snapshot<'a, P, S> {
    pub fn new(repo: &'a Repository<P, S>, snapshot: SnapshotFile) -> Result<Self> {
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
            trees: Vec::new(),
            tree,
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

    pub fn selected_node(&self) -> Option<&Node> {
        self.table.widget.selected().map(|i| &self.tree.nodes[i])
    }

    pub fn update_table(&mut self) {
        let old_selection = if self.tree.nodes.is_empty() {
            None
        } else {
            Some(self.table.widget.selected().unwrap_or_default())
        };
        let mut rows = Vec::new();
        let mut summary = Summary::default();
        for node in &self.tree.nodes {
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
                self.tree.nodes.len(),
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
        self.table.widget.select(old_selection);
    }

    pub fn enter(&mut self) -> Result<()> {
        if let Some(idx) = self.table.widget.selected() {
            let node = &self.tree.nodes[idx];
            if node.is_dir() {
                self.path.push(node.name());
                let tree = self.tree.clone();
                self.tree = self.repo.get_tree(&node.subtree.unwrap())?;
                self.trees.push(tree);
            }
        }
        self.table.widget.set_to(0);
        self.update_table();
        Ok(())
    }

    pub fn goback(&mut self) -> bool {
        _ = self.path.pop();
        if let Some(tree) = self.trees.pop() {
            self.tree = tree;
            self.table.widget.set_to(0);
            self.update_table();
            false
        } else {
            true
        }
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
                    Char('r') => {
                        if let Some(node) = self.selected_node() {
                            let path = self.path.join(node.name());
                            let restore = Restore::new(
                                self.repo,
                                node.clone(),
                                format!("{}:{}", self.snapshot.id, path.display()),
                            );
                            self.current_screen = CurrentScreen::Restore(restore);
                        }
                    }
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
            CurrentScreen::Restore(restore) => {
                if restore.input(event)? {
                    self.current_screen = CurrentScreen::Snapshot;
                }
            }
        }
        Ok(false)
    }

    pub fn draw(&mut self, area: Rect, f: &mut Frame<'_>) {
        let rects = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);

        if let CurrentScreen::Restore(restore) = &mut self.current_screen {
            restore.draw(area, f);
        } else {
            // draw the table
            self.table.draw(rects[0], f);

            // draw the footer
            let buffer_bg = tailwind::SLATE.c950;
            let row_fg = tailwind::SLATE.c200;
            let info_footer = Paragraph::new(Line::from(INFO_TEXT))
                .style(Style::new().fg(row_fg).bg(buffer_bg))
                .centered();
            f.render_widget(info_footer, rects[1]);
        }

        // draw popups
        match &mut self.current_screen {
            CurrentScreen::Snapshot | CurrentScreen::Restore(_) => {}
            CurrentScreen::ShowHelp(popup) => popup.draw(area, f),
        }
    }
}
