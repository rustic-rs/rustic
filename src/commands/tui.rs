//! `tui` subcommand
mod ls;
mod progress;
mod restore;
mod snapshots;
mod tree;
mod widgets;

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
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Arc::new(RwLock::new(Terminal::new(backend)?));

    let progress = TuiProgressBars {
        terminal: terminal.clone(),
    };
    let repo = open_repository_indexed_with_progress(&config.repository, progress)?;
    // create app and run it
    let snapshots = Snapshots::new(&repo, config.snapshot_filter.clone(), group_by)?;
    let app = App { snapshots };
    let res = run_app(terminal.clone(), app);

    // restore terminal
    disable_raw_mode()?;
    let mut terminal = terminal.write().unwrap();
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    drop(terminal);

    if let Err(err) = res {
        println!("{err:?}");
    }

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

        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                Char('q') | Esc => return Ok(()),
                _ => {}
            },
            _ => {}
        }
        app.snapshots.input(event)?;
    }
}

fn ui<P: ProgressBars, S: IndexedFull>(f: &mut Frame<'_>, app: &mut App<'_, P, S>) {
    let area = f.size();
    app.snapshots.draw(area, f);
}
