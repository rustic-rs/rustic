use super::*;

// Make a popup from a SizedWidget
pub struct PopUp<T>(pub T);

impl<T: ProcessEvent> ProcessEvent for PopUp<T> {
    type Result = T::Result;
    fn input(&mut self, event: Event) -> Self::Result {
        self.0.input(event)
    }
}

impl<T: Draw + SizedWidget> Draw for PopUp<T> {
    fn draw(&mut self, mut area: Rect, f: &mut Frame<'_>) {
        // center vertically
        if let Some(h) = self.0.height() {
            let layout = Layout::vertical([
                Constraint::Min(1),
                Constraint::Length(h),
                Constraint::Min(1),
            ]);
            area = layout.split(area)[1];
        }

        // center horizontally
        if let Some(w) = self.0.width() {
            let layout = Layout::horizontal([
                Constraint::Min(1),
                Constraint::Length(w),
                Constraint::Min(1),
            ]);
            area = layout.split(area)[1];
        }

        f.render_widget(Clear, area);
        self.0.draw(area, f)
    }
}
