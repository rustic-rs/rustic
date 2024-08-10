use super::*;

use crossterm::event::KeyModifiers;
use tui_textarea::TextArea;

pub struct TextInput {
    textarea: TextArea<'static>,
    lines: u16,
}

pub enum TextInputResult {
    Cancel,
    Input(String),
    None,
}

impl TextInput {
    pub fn new(text: &str, initial: &str, lines: u16) -> Self {
        let mut textarea = TextArea::default();
        textarea.set_style(Style::default());
        textarea.set_placeholder_text(text);
        _ = textarea.insert_str(initial);
        Self { textarea, lines }
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
            use KeyCode::*;
            match key {
                KeyEvent { code: Esc, .. } => return TextInputResult::Cancel,
                KeyEvent { code: Enter, .. } if self.lines == 1 => {
                    return TextInputResult::Input(self.textarea.lines().join("\n"));
                }
                KeyEvent {
                    code: Char('s'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    return TextInputResult::Input(self.textarea.lines().join("\n"));
                }
                key => {
                    _ = self.textarea.input(key);
                }
            }
        }
        TextInputResult::None
    }
}
