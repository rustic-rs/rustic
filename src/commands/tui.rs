//! `tui` subcommand
mod ls;
mod progress;
mod restore;
mod snapshots;
mod tree;
mod widgets;

use crossterm::event::{KeyEvent, KeyModifiers};
use progress::TuiProgressBars;
use snapshots::Snapshots;

use std::io;
use std::sync::{Arc, RwLock};

use crate::commands::open_repository_indexed_with_progress;
use crate::{Application, RUSTIC_APP};

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use rustic_core::{IndexedFull, ProgressBars, SnapshotGroupCriterion};

struct App<'a, P, S> {
    snapshots: Snapshots<'a, P, S>,
}

pub fn run(group_by: SnapshotGroupCriterion) -> Result<()> {
    let config = RUSTIC_APP.config();

    // setup terminal
    let terminal = init_terminal()?;
    let terminal = Arc::new(RwLock::new(terminal));

    let original_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |panic| {
        reset_terminal().unwrap();
        original_hook(panic);
    }));

    let progress = TuiProgressBars {
        terminal: terminal.clone(),
    };
    let repo = open_repository_indexed_with_progress(&config.repository, progress)?;
    // create app and run it
    let snapshots = Snapshots::new(&repo, config.snapshot_filter.clone(), group_by)?;
    let app = App { snapshots };
    let res = run_app(terminal, app);

    // restore terminal
    reset_terminal()?;

    if let Err(err) = res {
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

fn run_app<B: Backend, P: ProgressBars, S: IndexedFull>(
    terminal: Arc<RwLock<Terminal<B>>>,
    mut app: App<'_, P, S>,
) -> Result<()> {
    loop {
        _ = terminal.write().unwrap().draw(|f| ui(f, &mut app))?;
        let event = event::read()?;
        use KeyCode::*;

        if let Event::Key(KeyEvent {
            code: Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            ..
        }) = event
        {
            return Ok(());
        }
        if app.snapshots.input(event)? {
            return Ok(());
        }
    }
}

fn ui<P: ProgressBars, S: IndexedFull>(f: &mut Frame<'_>, app: &mut App<'_, P, S>) {
    let area = f.size();
    app.snapshots.draw(area, f);
}
