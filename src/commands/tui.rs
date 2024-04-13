//! `tui` subcommand
mod snapshots;
mod widgets;

use crossterm::event;
use snapshots::Snapshots;

use crate::{Application, RUSTIC_APP};

use abscissa_core::{status_err, Command, Runnable, Shutdown};

use anyhow::Result;
use std::io;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

use crate::commands::open_repository;

/// `tui` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct TuiCmd {}

impl Runnable for TuiCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

struct App {
    snapshots: Snapshots,
}

impl TuiCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();
        let repo = open_repository(&config.repository)?;

        // setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // create app and run it
        let snapshots = Snapshots::new(repo, config.snapshot_filter.clone())?;
        let app = App { snapshots };
        let res = run_app(&mut terminal, app);

        // restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        if let Err(err) = res {
            println!("{err:?}");
        }

        Ok(())
    }
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    loop {
        _ = terminal.draw(|f| ui(f, &mut app))?;
        let event = event::read()?;
        app.snapshots.input(event)?;
    }
}

fn ui(f: &mut Frame<'_>, app: &mut App) {
    let area = f.size();
    app.snapshots.draw(area, f);
}
