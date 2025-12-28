//! `rewrite` subcommand

use std::path::PathBuf;

use crate::{
    Application, RUSTIC_APP,
    repository::{CliOpenRepo, get_filtered_snapshots},
    status_err,
};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use clap::ValueHint;
use jiff::{Span, Zoned};
use rustic_core::{StringList, repofile::DeleteOption};

/// `rewrite` subcommand
#[derive(clap::Parser, Command, Debug, Default)]
pub(crate) struct RewriteCmd {
    /// Snapshots to rewrite. If none is given, use filter to filter from all snapshots.
    #[clap(value_name = "ID")]
    pub ids: Vec<String>,

    /// Set label
    #[clap(long, value_name = "LABEL", help_heading = "Snapshot options")]
    pub set_label: Option<String>,

    /// Set the backup time (e.g. "2021-01-21 14:15:23+0000")
    #[clap(long, help_heading = "Snapshot options")]
    pub set_time: Option<Zoned>,

    /// Set the host name
    #[clap(long, value_name = "NAME", help_heading = "Snapshot options")]
    pub set_hostname: Option<String>,

    /// Tags to add (can be specified multiple times)
    #[clap(
        long,
        value_name = "TAG[,TAG,..]",
        conflicts_with = "remove_tags",
        help_heading = "Tag options"
    )]
    pub add_tags: Vec<StringList>,

    /// Tag list to set (can be specified multiple times)
    #[clap(
        long,
        value_name = "TAG[,TAG,..]",
        conflicts_with = "remove_tags",
        help_heading = "Tag options"
    )]
    pub set_tags: Vec<StringList>,

    /// Tags to remove (can be specified multiple times)
    #[clap(long, value_name = "TAG[,TAG,..]", help_heading = "Tag options")]
    pub remove_tags: Vec<StringList>,

    /// Set description
    #[clap(long, value_name = "DESCRIPTION", help_heading = "Description options")]
    pub set_description: Option<String>,

    /// Read description to set from the given file
    #[clap(long, value_name = "FILE", conflicts_with = "set_description", value_hint = ValueHint::FilePath, help_heading = "Description options")]
    pub set_description_from: Option<PathBuf>,

    /// Remove description
    #[clap(
        long,
        conflicts_with_all = &["set_description", "set_description_from"], 
        help_heading = "Description options"
     )]
    pub remove_description: bool,

    /// Mark snapshot as uneraseable
    #[clap(
        long,
        conflicts_with = "set_delete_after",
        help_heading = "Delete mark options"
    )]
    pub set_delete_never: bool,

    /// Mark snapshot to be deleted after given duration (e.g. 10d)
    #[clap(long, value_name = "DURATION", help_heading = "Delete mark options")]
    pub set_delete_after: Option<Span>,

    /// Remove any delete mark
    #[clap(
        long,
        conflicts_with_all = &["set_delete_never", "set_delete_after"], 
        help_heading = "Delete mark options"
    )]
    pub remove_delete: bool,
}

impl Runnable for RewriteCmd {
    fn run(&self) {
        if let Err(err) = RUSTIC_APP
            .config()
            .repository
            .run_open(|repo| self.inner_run(repo))
        {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl RewriteCmd {
    fn inner_run(&self, repo: CliOpenRepo) -> Result<()> {
        let config = RUSTIC_APP.config();

        let snapshots = if self.ids.is_empty() {
            get_filtered_snapshots(&repo)?
        } else {
            repo.get_snapshots(&self.ids)?
        };

        let delete = match (
            self.remove_delete,
            self.set_delete_never,
            self.set_delete_after,
        ) {
            (true, _, _) => Some(DeleteOption::NotSet),
            (_, true, _) => Some(DeleteOption::Never),
            (_, _, Some(d)) => Some(DeleteOption::After(Zoned::now() + d)),
            (false, false, None) => None,
        };

        let description = match (self.remove_description, &self.set_description_from) {
            (true, _) => Some(None),
            (false, Some(path)) => Some(Some(std::fs::read_to_string(path)?)),
            (false, None) => self
                .set_description
                .as_ref()
                .map(|description| Some(description.clone())),
        };

        let snapshots: Vec<_> = snapshots
            .into_iter()
            .filter_map(|mut sn| {
                let mut changed = sn
                    .modify_sn(
                        self.set_tags.clone(),
                        self.add_tags.clone(),
                        &self.remove_tags,
                        &None, // TODO: Remove after modify_sn is refactored to modify_tags
                    )
                    .is_some();
                changed |= set_check(&mut sn.delete, &delete);
                changed |= set_check(&mut sn.label, &self.set_label);
                changed |= set_check(&mut sn.description, &description);
                changed |= set_check(&mut sn.time, &self.set_time);
                changed |= set_check(&mut sn.hostname, &self.set_hostname);
                changed.then_some(sn)
            })
            .collect();
        let old_snap_ids: Vec<_> = snapshots.iter().map(|sn| sn.id).collect();

        match (old_snap_ids.is_empty(), config.global.dry_run) {
            (true, _) => println!("no snapshot changed."),
            (false, true) => {
                println!("would have modified the following snapshots:\n {old_snap_ids:?}");
            }
            (false, false) => {
                repo.save_snapshots(snapshots)?;
                repo.delete_snapshots(&old_snap_ids)?;
            }
        }

        Ok(())
    }
}

fn set_check<T: PartialEq + Clone>(a: &mut T, b: &Option<T>) -> bool {
    if let Some(b) = b {
        if *a != *b {
            *a = b.clone();
            return true;
        }
    }
    false
}
