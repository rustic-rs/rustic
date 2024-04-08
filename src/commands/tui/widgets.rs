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
        let area = center_area(3, f.size()); // 1 (+2 for border)
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

pub struct PopUpTable {
    table: Table<'static>,
    height: u16,
}

impl PopUpTable {
    pub fn new(title: &'static str, rows: Vec<Vec<Text<'static>>>) -> Self {
        let height = rows
            .iter()
            .map(|row| row.iter().map(|t| t.height()).max().unwrap())
            .sum::<usize>()
            .try_into()
            .unwrap();

        let rows = rows.into_iter().map(Row::new);
        let table = Table::default()
            .block(Block::default().borders(Borders::ALL).title(title))
            // TODO: Apply to arbitrary column count; calculate widths
            .widths([Constraint::Length(15), Constraint::Min(0)])
            .rows(rows);
        Self { table, height }
    }

    pub fn render(&self, f: &mut Frame<'_>) {
        let area = center_area(self.height, f.size()); // +2 for block border
        f.render_widget(Clear, area);
        f.render_widget(&self.table, area);
    }
}

pub struct PopUpPrompt {
    p: Paragraph<'static>,
}

pub enum PopUpPromptResult {
    Ok,
    Cancel,
    None,
}

impl PopUpPrompt {
    pub fn new(title: &'static str, text: String) -> Self {
        let p = Paragraph::new(text).block(Block::default().borders(Borders::ALL).title(title));
        Self { p }
    }
    pub fn render(&self, f: &mut Frame<'_>) {
        let area = center_area(1, f.size());
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

fn center_area(height: u16, rect: Rect) -> Rect {
    let layout = Layout::default().constraints([
        Constraint::Min(0),
        Constraint::Length(height),
        Constraint::Min(0),
    ]);
    let chunks = layout.split(rect);
    chunks[1]
}
