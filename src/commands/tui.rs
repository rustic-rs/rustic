//! `tui` subcommand
mod diff;
mod ls;
mod progress;
mod restore;
mod snapshots;
pub mod summary;
mod tree;
mod widgets;

pub use diff::Diff;
pub use ls::Ls;
pub use snapshots::Snapshots;

use std::io;
use std::sync::{Arc, RwLock};

use anyhow::Result;
use crossterm::event::{KeyEvent, KeyModifiers};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use progress::TuiProgressBars;
use ratatui::prelude::*;
use scopeguard::defer;
use widgets::{Draw, ProcessEvent};

pub trait TuiResult {
    fn exit(&self) -> bool;
}

impl TuiResult for bool {
    fn exit(&self) -> bool {
        *self
    }
}

pub fn run(f: impl FnOnce(TuiProgressBars) -> Result<()>) -> Result<()> {
    // setup terminal
    let terminal = init_terminal()?;
    let terminal = Arc::new(RwLock::new(terminal));

    // restore terminal (even when leaving through ?, early return, or panic)
    defer! {
        reset_terminal().unwrap();
    }

    let progress = TuiProgressBars { terminal };

    if let Err(err) = f(progress) {
        println!("{err:?}");
    }

    Ok(())
}

/// Initializes the terminal.
fn init_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    enable_raw_mode()?;

    let backend = CrosstermBackend::new(io::stdout());

    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    Ok(terminal)
}

/// Resets the terminal.
fn reset_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}

pub fn run_app<T: TuiResult, A: Draw + ProcessEvent<Result = Result<T>>, B: Backend>(
    terminal: Arc<RwLock<Terminal<B>>>,
    mut app: A,
) -> Result<()> {
    loop {
        _ = terminal.write().unwrap().draw(|f| ui(f, &mut app))?;
        let event = event::read()?;

        if let Event::Key(KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            ..
        }) = event
        {
            return Ok(());
        }
        if app.input(event)?.exit() {
            return Ok(());
        }
    }
}

fn ui<A: Draw>(f: &mut Frame<'_>, app: &mut A) {
    let area = f.area();
    app.draw(area, f);
}
