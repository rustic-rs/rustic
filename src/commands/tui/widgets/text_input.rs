use super::*;

use crossterm::event::KeyModifiers;
use tui_textarea::{CursorMove, TextArea};

pub struct TextInput {
    textarea: TextArea<'static>,
    lines: u16,
    changeable: bool,
}

pub enum TextInputResult {
    Cancel,
    Input(String),
    None,
}

impl TextInput {
    pub fn new(text: Option<&str>, initial: &str, lines: u16, changeable: bool) -> Self {
        let mut textarea = TextArea::default();
        textarea.set_style(Style::default());
        if let Some(text) = text {
            textarea.set_placeholder_text(text);
        }
        _ = textarea.insert_str(initial);
        if !changeable {
            textarea.move_cursor(CursorMove::Top);
        }
        Self {
            textarea,
            lines,
            changeable,
        }
    }
}

impl SizedWidget for TextInput {
    fn height(&self) -> Option<u16> {
        Some(self.lines)
    }
}

impl Draw for TextInput {
    fn draw(&mut self, area: Rect, f: &mut Frame<'_>) {
        f.render_widget(&self.textarea, area);
    }
}

impl ProcessEvent for TextInput {
    type Result = TextInputResult;
    fn input(&mut self, event: Event) -> TextInputResult {
        if let Event::Key(key) = event {
            let KeyEvent {
                code, modifiers, ..
            } = key;
            use KeyCode::*;
            if self.changeable {
                match (code, modifiers) {
                    (Esc, _) => return TextInputResult::Cancel,
                    (Enter, _) if self.lines == 1 => {
                        return TextInputResult::Input(self.textarea.lines().join("\n"));
                    }
                    (Char('s'), KeyModifiers::CONTROL) => {
                        return TextInputResult::Input(self.textarea.lines().join("\n"));
                    }
                    _ => {
                        _ = self.textarea.input(key);
                    }
                }
            } else {
                match (code, modifiers) {
                    (Esc | Enter | Char('q') | Char('x'), _) => return TextInputResult::Cancel,
                    (Home, _) => {
                        self.textarea.move_cursor(CursorMove::Top);
                    }
                    (End, _) => {
                        self.textarea.move_cursor(CursorMove::Bottom);
                    }
                    (PageDown | PageUp | Up | Down, _) => {
                        _ = self.textarea.input(key);
                    }
                    _ => {}
                }
            }
        }
        TextInputResult::None
    }
}
