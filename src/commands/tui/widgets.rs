mod popup;
mod prompt;
mod sized_paragraph;
mod sized_table;
mod text_input;
mod with_block;

pub use popup::*;
pub use prompt::*;
pub use sized_paragraph::*;
pub use sized_table::*;
pub use text_input::*;
pub use with_block::*;

use crossterm::event::Event;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::prelude::*;
use ratatui::widgets::*;

pub trait ProcessEvent {
    type Result;
    fn input(&mut self, event: Event) -> Self::Result;
}

pub trait SizedWidget {
    fn height(&self) -> Option<u16> {
        None
    }
    fn width(&self) -> Option<u16> {
        None
    }
}

pub trait Draw {
    fn draw(&mut self, area: Rect, f: &mut Frame<'_>);
}
