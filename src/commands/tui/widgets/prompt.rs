use super::{Draw, Event, Frame, KeyCode, KeyEventKind, ProcessEvent, Rect, SizedWidget};

pub struct Prompt<T>(pub T);

pub enum PromptResult {
    Ok,
    Cancel,
    None,
}

impl<T: SizedWidget> SizedWidget for Prompt<T> {
    fn height(&self) -> Option<u16> {
        self.0.height()
    }
    fn width(&self) -> Option<u16> {
        self.0.width()
    }
}

impl<T: Draw> Draw for Prompt<T> {
    fn draw(&mut self, area: Rect, f: &mut Frame<'_>) {
        self.0.draw(area, f);
    }
}

impl<T> ProcessEvent for Prompt<T> {
    type Result = PromptResult;
    fn input(&mut self, event: Event) -> PromptResult {
        use KeyCode::{Char, Enter, Esc};
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                Char('q' | 'n' | 'c') | Esc => PromptResult::Cancel,
                Enter | Char('y' | 'j' | ' ') => PromptResult::Ok,
                _ => PromptResult::None,
            },
            _ => PromptResult::None,
        }
    }
}
