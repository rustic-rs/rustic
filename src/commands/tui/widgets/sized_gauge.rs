use super::*;

pub struct SizedGauge {
    p: Gauge<'static>,
    width: Option<u16>,
}

impl SizedGauge {
    pub fn new(text: Span<'static>, ratio: f64) -> Self {
        let width = text.width().try_into().ok();
        let p = Gauge::default()
            .gauge_style(Style::default().fg(Color::Blue))
            .use_unicode(true)
            .label(text)
            .ratio(ratio);
        Self { p, width }
    }
}

impl SizedWidget for SizedGauge {
    fn width(&self) -> Option<u16> {
        self.width.map(|w| w + 10)
    }
    fn height(&self) -> Option<u16> {
        Some(1)
    }
}

impl Draw for SizedGauge {
    fn draw(&mut self, area: Rect, f: &mut Frame<'_>) {
        f.render_widget(&self.p, area);
    }
}
