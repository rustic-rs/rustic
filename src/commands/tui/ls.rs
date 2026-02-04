use std::path::{Path, PathBuf};

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};
use rustic_core::{
    IndexedFull, Progress, ProgressBars, Repository, TreeId,
    repofile::{Node, SnapshotFile, Tree},
};
use style::palette::tailwind;

use crate::{
    commands::{
        ls::{NodeLs, Summary},
        tui::{
            TuiResult,
            restore::Restore,
            widgets::{
                Draw, PopUpPrompt, PopUpTable, PopUpText, ProcessEvent, PromptResult, SelectTable,
                TextInputResult, WithBlock, popup_prompt, popup_scrollable_text, popup_table,
                popup_text,
            },
        },
    },
    helpers::bytes_size_to_string,
};

use super::{summary::SummaryMap, widgets::PopUpInput};

// the states this screen can be in
enum CurrentScreen<'a, P, S> {
    Ls,
    ShowHelp(PopUpText),
    Table(PopUpTable),
    Restore(Box<Restore<'a, P, S>>),
    PromptExit(PopUpPrompt),
    PromptLeave(PopUpPrompt),
    ShowFile(Box<PopUpInput>),
}

const INFO_TEXT: &str = "(Esc) quit | (Enter) enter dir | (Backspace) return to parent | (v) view | (r) restore | (?) show all commands";

const HELP_TEXT: &str = r"
Ls Commands:

          v : view file contents (text files only, up to 1MiB)
          r : restore selected item
          n : toggle numeric IDs
          s : compute information for (sub-)dirs and show summary
          S : compute information for selected node and show summary
          D : diff current selection

General Commands:

      q,Esc : exit
      Enter : enter dir
  Backspace : return to parent dir
          ? : show this help page

 ";

pub struct Ls<'a, P, S> {
    current_screen: CurrentScreen<'a, P, S>,
    numeric: bool,
    table: WithBlock<SelectTable>,
    repo: &'a Repository<P, S>,
    snapshot: SnapshotFile,
    path: PathBuf,
    trees: Vec<(Tree, TreeId, usize)>, // Stack of parent trees with position
    tree: Tree,
    tree_id: TreeId,
    summary_map: SummaryMap,
}

pub enum LsResult {
    Exit,
    Return(SummaryMap),
    None,
}

impl TuiResult for LsResult {
    fn exit(&self) -> bool {
        !matches!(self, Self::None)
    }
}

impl<'a, P: ProgressBars, S: IndexedFull> Ls<'a, P, S> {
    pub fn new(
        repo: &'a Repository<P, S>,
        snapshot: SnapshotFile,
        path: &str,
        summary_map: SummaryMap,
    ) -> Result<Self> {
        let header = ["Name", "Size", "Mode", "User", "Group", "Time"]
            .into_iter()
            .map(Text::from)
            .collect();

        let node = repo.node_from_snapshot_and_path(&snapshot, path)?;
        let (tree_id, tree) = node.subtree.map_or_else(
            || -> Result<_> {
                Ok((
                    TreeId::default(),
                    Tree {
                        nodes: vec![node.clone()],
                    },
                ))
            },
            |id| Ok((id, repo.get_tree(&id)?)),
        )?;
        let mut app = Self {
            current_screen: CurrentScreen::Ls,
            numeric: false,
            table: WithBlock::new(SelectTable::new(header), Block::new()),
            repo,
            snapshot,
            path: PathBuf::from(path),
            trees: Vec::new(),
            tree,
            tree_id,
            summary_map,
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
        let size = bytes_size_to_string(node.meta.size);
        let mtime = node.meta.mtime.map_or_else(
            || "?".to_string(),
            |t| format!("{}", t.strftime("%Y-%m-%d %H:%M:%S")),
        );
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
            let mut node = node.clone();
            if node.is_dir() {
                let id = node.subtree.unwrap();
                if let Some(sum) = self.summary_map.get(&id) {
                    summary += sum.summary;
                    node.meta.size = sum.summary.size;
                } else {
                    summary.update(&node);
                }
            } else {
                summary.update(&node);
            }
            let row = self.ls_row(&node);
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
                let tree_id = self.tree_id;
                self.tree_id = node.subtree.unwrap();
                self.tree = self.repo.get_tree(&self.tree_id)?;
                self.trees.push((tree, tree_id, idx));
            }
        }
        self.table.widget.set_to(0);
        self.update_table();
        Ok(())
    }

    pub fn goback(&mut self) {
        _ = self.path.pop();
        if let Some((tree, tree_id, idx)) = self.trees.pop() {
            self.tree = tree;
            self.tree_id = tree_id;
            self.table.widget.set_to(idx);
            self.update_table();
        }
    }

    pub fn in_root(&self) -> bool {
        self.trees.is_empty()
    }

    pub fn toggle_numeric(&mut self) {
        self.numeric = !self.numeric;
        self.update_table();
    }

    pub fn compute_summary(&mut self, tree_id: TreeId) -> Result<()> {
        let pb = self.repo.progress_bars();
        let p = pb.progress_counter("computing (sub)-dir information");
        self.summary_map.compute(self.repo, tree_id, &p)?;
        p.finish();
        self.update_table();
        Ok(())
    }

    pub fn summary(&mut self) -> Result<PopUpTable> {
        // Compute and show summary
        self.compute_summary(self.tree_id)?;
        let header = format!("{}:{}", self.snapshot.id, self.path.display());
        let mut stats = self
            .summary_map
            .compute_statistics(&self.tree.nodes, self.repo)?;
        // Current dir
        stats.summary.dirs += 1;

        let rows = stats.table(header);
        Ok(popup_table("summary", rows))
    }

    pub fn summary_selected(&mut self) -> Result<Option<PopUpTable>> {
        let Some(selected) = self.table.widget.selected() else {
            return Ok(None);
        };
        // Compute and show summary
        self.compute_summary(self.tree_id)?;
        let node = &self.tree.nodes[selected];
        let header = format!(
            "{}:{}",
            self.snapshot.id,
            self.path.join(node.name()).display()
        );
        let stats = self.summary_map.compute_statistics(Some(node), self.repo)?;

        let rows = stats.table(header);
        Ok(Some(popup_table("summary", rows)))
    }
}

impl<'a, P: ProgressBars, S: IndexedFull> ProcessEvent for Ls<'a, P, S> {
    type Result = Result<LsResult>;
    fn input(&mut self, event: Event) -> Result<LsResult> {
        use KeyCode::{Backspace, Char, Enter, Esc, Left, Right};
        match &mut self.current_screen {
            CurrentScreen::Ls => match event {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    Enter | Right => self.enter()?,
                    Backspace | Left => {
                        if self.in_root() {
                            self.current_screen = CurrentScreen::PromptLeave(popup_prompt(
                                "leave ls",
                                "do you want to leave the ls view? (y/n)".into(),
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
                    Char('n') => self.toggle_numeric(),
                    Char('s') => {
                        self.current_screen = CurrentScreen::Table(self.summary()?);
                    }
                    Char('S') => {
                        if let Some(table) = self.summary_selected()? {
                            self.current_screen = CurrentScreen::Table(table);
                        }
                    }
                    Char('v') => {
                        // viewing is not supported on cold repositories
                        if self.repo.config().is_hot != Some(true)
                            && let Some(node) = self.selected_node()
                            && node.is_file()
                            && let Ok(data) = self.repo.open_file(node)?.read_at(
                                self.repo,
                                0,
                                node.meta.size.min(1_000_000).try_into().unwrap(),
                            )
                        {
                            // viewing is only supported for text files
                            if let Ok(content) = String::from_utf8(data.to_vec()) {
                                let lines = content.lines().count();
                                let path = self.path.join(node.name());
                                let path = path.display();
                                self.current_screen =
                                    CurrentScreen::ShowFile(Box::new(popup_scrollable_text(
                                        format!("{}:/{path}", self.snapshot.id),
                                        &content,
                                        (lines + 1).min(40).try_into().unwrap(),
                                    )));
                            }
                        }
                    }
                    Char('r') => {
                        if let Some(node) = self.selected_node() {
                            let is_absolute = self
                                .snapshot
                                .paths
                                .iter()
                                .any(|p| Path::new(p).is_absolute());
                            let path = self.path.join(node.name());
                            let path = path.display();
                            let default_target = if is_absolute {
                                format!("/{path}")
                            } else {
                                format!("{path}")
                            };
                            let restore = Restore::new(
                                self.repo,
                                node.clone(),
                                format!("{}:/{path}", self.snapshot.id),
                                &default_target,
                            );
                            self.current_screen = CurrentScreen::Restore(Box::new(restore));
                        }
                    }
                    _ => self.table.input(event),
                },
                _ => {}
            },
            CurrentScreen::ShowFile(prompt) => match prompt.input(event) {
                TextInputResult::Cancel | TextInputResult::Input(_) => {
                    self.current_screen = CurrentScreen::Ls;
                }
                TextInputResult::None => {}
            },
            CurrentScreen::Table(_) | CurrentScreen::ShowHelp(_) => match event {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if matches!(key.code, Char('q' | ' ' | '?') | Esc | Enter) {
                        self.current_screen = CurrentScreen::Ls;
                    }
                }
                _ => {}
            },
            CurrentScreen::Restore(restore) => {
                if restore.input(event)? {
                    self.current_screen = CurrentScreen::Ls;
                }
            }
            CurrentScreen::PromptExit(prompt) => match prompt.input(event) {
                PromptResult::Ok => return Ok(LsResult::Exit),
                PromptResult::Cancel => self.current_screen = CurrentScreen::Ls,
                PromptResult::None => {}
            },
            CurrentScreen::PromptLeave(prompt) => match prompt.input(event) {
                PromptResult::Ok => {
                    return Ok(LsResult::Return(std::mem::take(&mut self.summary_map)));
                }
                PromptResult::Cancel => self.current_screen = CurrentScreen::Ls,
                PromptResult::None => {}
            },
        }
        Ok(LsResult::None)
    }
}

impl<'a, P: ProgressBars, S: IndexedFull> Draw for Ls<'a, P, S> {
    fn draw(&mut self, area: Rect, f: &mut Frame<'_>) {
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
            CurrentScreen::Ls | CurrentScreen::Restore(_) => {}
            CurrentScreen::Table(popup) => popup.draw(area, f),
            CurrentScreen::ShowHelp(popup) => popup.draw(area, f),
            CurrentScreen::PromptExit(popup) | CurrentScreen::PromptLeave(popup) => {
                popup.draw(area, f);
            }
            CurrentScreen::ShowFile(popup) => popup.draw(area, f),
        }
    }
}
