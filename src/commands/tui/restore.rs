use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::prelude::*;
use rustic_core::{
    IndexedFull, LocalDestination, LsOptions, ProgressBars, Repository, RestoreOptions,
    RestorePlan, repofile::Node,
};

use crate::{
    commands::tui::widgets::{
        Draw, PopUpInput, PopUpPrompt, PopUpText, ProcessEvent, PromptResult, TextInputResult,
        popup_input, popup_prompt,
    },
    helpers::bytes_size_to_string,
};

use super::widgets::popup_text;

// the states this screen can be in
enum CurrentScreen {
    GetDestination(PopUpInput),
    PromptRestore(PopUpPrompt, Option<RestorePlan>),
    RestoreDone(PopUpText),
}

pub(crate) struct Restore<'a, P, S> {
    current_screen: CurrentScreen,
    repo: &'a Repository<P, S>,
    opts: RestoreOptions,
    node: Node,
    source: String,
    dest: String,
}

impl<'a, P: ProgressBars, S: IndexedFull> Restore<'a, P, S> {
    pub fn new(repo: &'a Repository<P, S>, node: Node, source: String, path: &str) -> Self {
        let opts = RestoreOptions::default();
        let title = format!("restore {source} to:");
        let popup = popup_input(title, "enter restore destination", path, 1);
        Self {
            current_screen: CurrentScreen::GetDestination(popup),
            node,
            repo,
            opts,
            source,
            dest: String::new(),
        }
    }

    pub fn compute_plan(&mut self, mut dest: String, dry_run: bool) -> Result<RestorePlan> {
        if dest.is_empty() {
            dest = ".".to_string();
        }
        self.dest = dest;
        let dest = LocalDestination::new(&self.dest, true, !self.node.is_dir())?;

        // for restore, always recurse into tree
        let mut ls_opts = LsOptions::default();
        ls_opts.recursive = true;

        let ls = self.repo.ls(&self.node, &ls_opts)?;

        let plan = self.repo.prepare_restore(&self.opts, ls, &dest, dry_run)?;

        Ok(plan)
    }

    // restore using the plan
    //
    // Note: This currently runs `prepare_restore` again and doesn't use `plan`
    // TODO: Fix when restore is changed such that `prepare_restore` is always dry_run and all modification is done in `restore`
    fn restore(&self, _plan: RestorePlan) -> Result<()> {
        let dest = LocalDestination::new(&self.dest, true, !self.node.is_dir())?;

        // for restore, always recurse into tree
        let mut ls_opts = LsOptions::default();
        ls_opts.recursive = true;

        let ls = self.repo.ls(&self.node, &ls_opts)?;
        let plan = self
            .repo
            .prepare_restore(&self.opts, ls.clone(), &dest, false)?;

        // the actual restore
        self.repo.restore(plan, &self.opts, ls, &dest)?;
        Ok(())
    }

    pub fn input(&mut self, event: Event) -> Result<bool> {
        use KeyCode::{Char, Enter, Esc};
        match &mut self.current_screen {
            CurrentScreen::GetDestination(prompt) => match prompt.input(event) {
                TextInputResult::Cancel => return Ok(true),
                TextInputResult::Input(input) => {
                    let plan = self.compute_plan(input, true)?;
                    let fs = plan.stats.files;
                    let ds = plan.stats.dirs;
                    let popup = popup_prompt(
                        "restore information",
                        Text::from(format!(
                            r#"
restoring from: {}
restoring to: {}
                            
Files:  {} to restore, {} unchanged, {} verified, {} to modify, {} additional
Dirs:   {} to restore, {} to modify, {} additional
Total restore size: {}

Do you want to proceed (y/n)?
 "#,
                            self.source,
                            self.dest,
                            fs.restore,
                            fs.unchanged,
                            fs.verified,
                            fs.modify,
                            fs.additional,
                            ds.restore,
                            ds.modify,
                            ds.additional,
                            bytes_size_to_string(plan.restore_size)
                        )),
                    );
                    self.current_screen = CurrentScreen::PromptRestore(popup, Some(plan));
                }
                TextInputResult::None => {}
            },
            CurrentScreen::PromptRestore(prompt, plan) => match prompt.input(event) {
                PromptResult::Ok => {
                    let plan = plan.take().unwrap();
                    self.restore(plan)?;
                    self.current_screen = CurrentScreen::RestoreDone(popup_text(
                        "restore done",
                        format!("restored {} successfully to {}", self.source, self.dest).into(),
                    ));
                }
                PromptResult::Cancel => return Ok(true),
                PromptResult::None => {}
            },
            CurrentScreen::RestoreDone(_) => match event {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if matches!(key.code, Char('q' | ' ') | Esc | Enter) {
                        return Ok(true);
                    }
                }
                _ => {}
            },
        }
        Ok(false)
    }

    pub fn draw(&mut self, area: Rect, f: &mut Frame<'_>) {
        // draw popups
        match &mut self.current_screen {
            CurrentScreen::GetDestination(popup) => popup.draw(area, f),
            CurrentScreen::PromptRestore(popup, _) => popup.draw(area, f),
            CurrentScreen::RestoreDone(popup) => popup.draw(area, f),
        }
    }
}
