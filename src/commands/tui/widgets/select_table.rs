use super::{
    layout, style, Color, Constraint, Draw, Event, Frame, KeyCode, KeyEventKind, Layout, Modifier,
    ProcessEvent, Rect, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, SizedWidget, Style,
    Stylize, Table, TableState, Text,
};
use std::iter::once;
use style::palette::tailwind;

struct TableColors {
    buffer_bg: Color,
    header_bg: Color,
    header_fg: Color,
    row_fg: Color,
    selected_style_fg: Color,
    normal_row_color: Color,
    alt_row_color: Color,
}

impl TableColors {
    fn new(color: &tailwind::Palette) -> Self {
        Self {
            buffer_bg: tailwind::SLATE.c950,
            header_bg: color.c900,
            header_fg: tailwind::SLATE.c200,
            row_fg: tailwind::SLATE.c200,
            selected_style_fg: color.c400,
            normal_row_color: tailwind::SLATE.c950,
            alt_row_color: tailwind::SLATE.c900,
        }
    }
}

pub struct SelectTable {
    header: Vec<Text<'static>>,
    table: Table<'static>,
    state: TableState,
    scroll_state: ScrollbarState,
    rows: usize,
    rows_display: usize,
    row_height: usize,
}

impl SelectTable {
    pub fn new(header: Vec<Text<'static>>) -> Self {
        let table = Table::default();

        Self {
            header,
            table,
            state: TableState::default(),
            scroll_state: ScrollbarState::new(0),
            rows: 0,
            rows_display: 0,
            row_height: 0,
        }
    }

    pub fn set_content(&mut self, content: Vec<Vec<Text<'static>>>, row_height: usize) {
        let colors = TableColors::new(&tailwind::BLUE);
        let selected_style = Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(colors.selected_style_fg);

        let header_style = Style::default().fg(colors.header_fg).bg(colors.header_bg);

        self.row_height = row_height;
        let widths = once(&self.header)
            .chain(content.iter())
            .map(|row| row.iter().map(Text::width).collect())
            .reduce(|widths: Vec<usize>, row| {
                row.iter()
                    .zip(widths.iter())
                    .map(|(r, w)| r.max(w))
                    .copied()
                    .collect()
            })
            .unwrap_or_default();

        self.rows = content.len();
        self.scroll_state = ScrollbarState::new(self.rows * self.row_height);

        let content = content.into_iter().enumerate().map(|(i, row)| {
            let color = match i % 2 {
                0 => colors.normal_row_color,
                _ => colors.alt_row_color,
            };
            Row::new(row)
                .style(Style::new().fg(colors.row_fg).bg(color))
                .height(self.row_height.try_into().unwrap())
        });

        self.table = Table::default()
            .header(Row::new(self.header.clone()).style(header_style))
            .row_highlight_style(selected_style)
            .bg(colors.buffer_bg)
            .widths(widths.iter().map(|w| {
                (*w).try_into()
                    .ok()
                    .map_or(Constraint::Min(0), Constraint::Length)
            }))
            .flex(layout::Flex::SpaceBetween)
            .rows(content);
    }

    pub fn selected(&self) -> Option<usize> {
        self.state.selected()
    }

    pub fn select(&mut self, index: Option<usize>) {
        self.state.select(index);
    }

    pub fn set_to(&mut self, i: usize) {
        self.state.select(Some(i));
        self.scroll_state = self.scroll_state.position(i * self.row_height);
    }

    pub fn go_forward(&mut self, step: usize) {
        if let Some(selected_old) = self.state.selected() {
            let selected = (selected_old + step).min(self.rows - 1);
            self.set_to(selected);
        }
    }

    pub fn go_back(&mut self, step: usize) {
        if let Some(selected_old) = self.state.selected() {
            let selected = selected_old.saturating_sub(step);
            self.set_to(selected);
        }
    }

    pub fn next(&mut self) {
        self.go_forward(1);
    }

    pub fn page_down(&mut self) {
        self.go_forward(self.rows_display);
    }

    pub fn previous(&mut self) {
        self.go_back(1);
    }

    pub fn page_up(&mut self) {
        self.go_back(self.rows_display);
    }

    pub fn home(&mut self) {
        if self.state.selected().is_some() {
            self.set_to(0);
        }
    }

    pub fn end(&mut self) {
        if self.state.selected().is_some() {
            self.set_to(self.rows - 1);
        }
    }

    pub fn set_rows(&mut self, rows: usize) {
        self.rows_display = rows / self.row_height;
    }
}

impl ProcessEvent for SelectTable {
    type Result = ();
    fn input(&mut self, event: Event) {
        use KeyCode::{Down, End, Home, PageDown, PageUp, Up};
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                Down => self.next(),
                Up => self.previous(),
                PageDown => self.page_down(),
                PageUp => self.page_up(),
                Home => self.home(),
                End => self.end(),
                _ => {}
            },
            _ => {}
        }
    }
}

impl SizedWidget for SelectTable {}

impl Draw for SelectTable {
    fn draw(&mut self, area: Rect, f: &mut Frame<'_>) {
        self.set_rows(area.height.into());
        let chunks = Layout::horizontal([Constraint::Min(0), Constraint::Length(1)]).split(area);
        f.render_stateful_widget(&self.table, chunks[0], &mut self.state);
        f.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None),
            chunks[1],
            &mut self.scroll_state,
        );
    }
}
