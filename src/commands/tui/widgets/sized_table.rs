use super::{Constraint, Draw, Frame, Rect, Row, SizedWidget, Table, Text};

pub struct SizedTable {
    table: Table<'static>,
    height: usize,
    width: usize,
}

impl SizedTable {
    pub fn new(content: Vec<Vec<Text<'static>>>) -> Self {
        let height = content
            .iter()
            .map(|row| row.iter().map(Text::height).max().unwrap_or_default())
            .sum::<usize>();

        let widths = content
            .iter()
            .map(|row| row.iter().map(Text::width).collect())
            .reduce(|widths: Vec<usize>, row| {
                row.iter()
                    .zip(widths.iter())
                    .map(|(r, w)| r.max(w))
                    .copied()
                    .collect()
            })
            .unwrap_or_default();

        let width = widths
            .iter()
            .copied()
            .reduce(|width, w| width + w + 1) // +1 because of space between entries
            .unwrap_or_default();

        let rows = content.into_iter().map(Row::new);
        let table = Table::default()
            .widths(widths.iter().map(|w| {
                (*w).try_into()
                    .ok()
                    .map_or(Constraint::Min(0), Constraint::Length)
            }))
            .rows(rows);
        Self {
            table,
            height,
            width,
        }
    }
}

impl SizedWidget for SizedTable {
    fn height(&self) -> Option<u16> {
        self.height.try_into().ok()
    }
    fn width(&self) -> Option<u16> {
        self.width.try_into().ok()
    }
}

impl Draw for SizedTable {
    fn draw(&mut self, area: Rect, f: &mut Frame<'_>) {
        f.render_widget(&self.table, area);
    }
}
