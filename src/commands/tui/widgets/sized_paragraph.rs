use super::*;

pub struct SizedParagraph {
    p: Paragraph<'static>,
    height: Option<u16>,
    width: Option<u16>,
}

impl SizedParagraph {
    pub fn new(text: Text<'static>) -> Self {
        let height = text.height().try_into().ok();
        let width = text.width().try_into().ok();
        let p = Paragraph::new(text);
        Self { p, height, width }
    }
}

impl SizedWidget for SizedParagraph {
    fn width(&self) -> Option<u16> {
        self.width
    }
    fn height(&self) -> Option<u16> {
        self.height
    }
}

impl Draw for SizedParagraph {
    fn draw(&mut self, area: Rect, f: &mut Frame<'_>) {
        f.render_widget(&self.p, area);
    }
}
