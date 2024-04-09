use crossterm::event::Event;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::prelude::*;
use ratatui::widgets::*;
use tui_textarea::TextArea;

pub struct PopUpInput {
    textarea: TextArea<'static>,
}

pub enum PopUpInputResult {
    Cancel,
    Input(String),
    None,
}

impl PopUpInput {
    pub fn new(text: &str, title: &'static str, initial: &str) -> Self {
        let mut textarea = TextArea::default();
        textarea.set_style(Style::default());
        textarea.set_block(Block::default().borders(Borders::ALL).title(title));
        textarea.set_placeholder_text(text);
        _ = textarea.insert_str(initial);
        Self { textarea }
    }

    pub fn render(&self, f: &mut Frame<'_>) {
        let area = center_area(Some(1), None, f.size());
        f.render_widget(Clear, area);
        f.render_widget(self.textarea.widget(), area);
    }

    pub fn input(&mut self, event: Event) -> PopUpInputResult {
        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return PopUpInputResult::None;
            }
            use KeyCode::*;
            match key {
                KeyEvent { code: Esc, .. } => return PopUpInputResult::Cancel,
                KeyEvent { code: Enter, .. } => {
                    return PopUpInputResult::Input(self.textarea.lines()[0].clone());
                }
                key => {
                    _ = self.textarea.input(key);
                }
            }
        }
        PopUpInputResult::None
    }
}

pub struct PopUpParagraph {
    p: Paragraph<'static>,
    height: Option<u16>,
    width: Option<u16>,
}

impl PopUpParagraph {
    pub fn new(title: &'static str, rows: Text<'static>) -> Self {
        let height = rows.height().try_into().ok();
        let width = rows.width().try_into().ok();
        let p = Paragraph::new(rows).block(Block::default().borders(Borders::ALL).title(title));
        Self { p, height, width }
    }

    pub fn render(&self, f: &mut Frame<'_>) {
        let area = center_area(self.height, self.width, f.size()); // +2 for block border
        f.render_widget(Clear, area);
        f.render_widget(&self.p, area);
    }
}

pub struct PopUpTable {
    table: Table<'static>,
    height: Option<u16>,
    width: Option<u16>,
}

impl PopUpTable {
    pub fn new(title: &'static str, rows: Vec<Vec<Text<'static>>>) -> Self {
        let height = rows
            .iter()
            .map(|row| row.iter().map(Text::height).max().unwrap_or_default())
            .sum::<usize>()
            .try_into()
            .ok();

        let widths = rows
            .iter()
            .map(|row| row.iter().map(Text::width).collect())
            .reduce(|widths: Vec<usize>, row| {
                row.iter()
                    .zip(widths.iter())
                    .map(|(r, w)| r.max(w))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();

        let width = widths
            .iter()
            .cloned()
            .reduce(|width, w| width + w + 1)
            .unwrap_or_default()
            .try_into()
            .ok();
        let rows = rows.into_iter().map(Row::new);
        let table = Table::default()
            .block(Block::default().borders(Borders::ALL).title(title))
            .widths(widths.iter().map(|w| {
                (*w).try_into()
                    .ok()
                    .map_or(Constraint::Min(0), Constraint::Length)
            }))
            .rows(rows);
        Self {
            table,
            height,
            width,
        }
    }

    pub fn render(&self, f: &mut Frame<'_>) {
        let area = center_area(self.height, self.width, f.size());
        f.render_widget(Clear, area);
        f.render_widget(&self.table, area);
    }
}

pub struct PopUpPrompt {
    p: Paragraph<'static>,
    width: Option<u16>,
}

pub enum PopUpPromptResult {
    Ok,
    Cancel,
    None,
}

impl PopUpPrompt {
    pub fn new(title: &'static str, text: String) -> Self {
        let width = text.len().try_into().ok();
        let p = Paragraph::new(text).block(Block::default().borders(Borders::ALL).title(title));
        Self { p, width }
    }
    pub fn render(&self, f: &mut Frame<'_>) {
        let area = center_area(Some(1), self.width, f.size());
        f.render_widget(Clear, area);
        f.render_widget(&self.p, area);
    }
    pub fn input(&mut self, event: Event) -> PopUpPromptResult {
        use KeyCode::*;
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                Char('q') | Char('n') | Char('c') | Esc => PopUpPromptResult::Cancel,
                Enter | Char('y') | Char('j') | Char(' ') => PopUpPromptResult::Ok,
                _ => PopUpPromptResult::None,
            },
            _ => PopUpPromptResult::None,
        }
    }
}

fn center_area(height: Option<u16>, width: Option<u16>, rect: Rect) -> Rect {
    let layout = Layout::vertical([
        Constraint::Min(0),
        height.map_or(Constraint::Percentage(100), |h| Constraint::Length(h + 2)), // +2 for block border
        Constraint::Min(0),
    ]);
    let chunks = layout.split(rect);
    let layout = Layout::horizontal([
        Constraint::Min(1),
        width.map_or(Constraint::Percentage(100), |h| Constraint::Length(h + 2)), // +2 for block border
        Constraint::Min(1),
    ]);
    let chunks = layout.split(chunks[1]);
    chunks[1]
}
