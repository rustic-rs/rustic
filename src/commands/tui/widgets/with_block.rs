use super::*;
use layout::Size;

pub struct WithBlock<T> {
    pub block: Block<'static>,
    pub widget: T,
}

impl<T> WithBlock<T> {
    pub fn new(widget: T, block: Block<'static>) -> Self {
        Self { block, widget }
    }

    // Note: this could be a method of self.block, but is unfortunately not present
    // So we compute ourselves using self.block.inner() on an artificial Rect.
    fn size_diff(&self) -> Size {
        let rect = Rect {
            x: 0,
            y: 0,
            width: u16::MAX,
            height: u16::MAX,
        };
        let inner = self.block.inner(rect);
        Size {
            width: rect.as_size().width - inner.as_size().width,
            height: rect.as_size().height - inner.as_size().height,
        }
    }
}

impl<T: ProcessEvent> ProcessEvent for WithBlock<T> {
    type Result = T::Result;
    fn input(&mut self, event: Event) -> Self::Result {
        self.widget.input(event)
    }
}

impl<T: SizedWidget> SizedWidget for WithBlock<T> {
    fn height(&self) -> Option<u16> {
        self.widget
            .height()
            .map(|h| h.saturating_add(self.size_diff().height))
    }

    fn width(&self) -> Option<u16> {
        self.widget
            .width()
            .map(|w| w.saturating_add(self.size_diff().width))
    }
}

impl<T: Draw + SizedWidget> Draw for WithBlock<T> {
    fn draw(&mut self, area: Rect, f: &mut Frame<'_>) {
        f.render_widget(self.block.clone(), area);
        self.widget.draw(self.block.inner(area), f);
    }
}
