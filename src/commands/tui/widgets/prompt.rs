use super::*;

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
        use KeyCode::*;
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                Char('q') | Char('n') | Char('c') | Esc => PromptResult::Cancel,
                Enter | Char('y') | Char('j') | Char(' ') => PromptResult::Ok,
                _ => PromptResult::None,
            },
            _ => PromptResult::None,
        }
    }
}
