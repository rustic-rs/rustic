use super::*;

use tui_textarea::TextArea;

pub struct TextInput {
    textarea: TextArea<'static>,
}

pub enum TextInputResult {
    Cancel,
    Input(String),
    None,
}

impl TextInput {
    pub fn new(text: &str, initial: &str) -> Self {
        let mut textarea = TextArea::default();
        textarea.set_style(Style::default());
        textarea.set_placeholder_text(text);
        _ = textarea.insert_str(initial);
        Self { textarea }
    }
}

impl SizedWidget for TextInput {
    fn height(&self) -> Option<u16> {
        Some(1)
    }
}

impl Draw for TextInput {
    fn draw(&mut self, area: Rect, f: &mut Frame<'_>) {
        f.render_widget(self.textarea.widget(), area);
    }
}

impl ProcessEvent for TextInput {
    type Result = TextInputResult;
    fn input(&mut self, event: Event) -> TextInputResult {
        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return TextInputResult::None;
            }
            use KeyCode::*;
            match key {
                KeyEvent { code: Esc, .. } => return TextInputResult::Cancel,
                KeyEvent { code: Enter, .. } => {
                    return TextInputResult::Input(self.textarea.lines()[0].clone());
                }
                key => {
                    _ = self.textarea.input(key);
                }
            }
        }
        TextInputResult::None
    }
}
