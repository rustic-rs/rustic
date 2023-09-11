//! `tag` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{commands::open_repository, status_err, Application, RUSTIC_APP};

use abscissa_core::{Command, Runnable, Shutdown};

use chrono::{Duration, Local};

use rustic_core::{repofile::DeleteOption, StringList};

/// `tag` subcommand

#[derive(clap::Parser, Command, Debug)]
pub(crate) struct TagCmd {
    /// Snapshots to change tags. If none is given, use filter to filter from all
    /// snapshots.
    #[clap(value_name = "ID")]
    ids: Vec<String>,

    /// Tags to add (can be specified multiple times)
    #[clap(
        long,
        value_name = "TAG[,TAG,..]",
        conflicts_with = "remove",
        help_heading = "Tag options"
    )]
    add: Vec<StringList>,

    /// Tags to remove (can be specified multiple times)
    #[clap(long, value_name = "TAG[,TAG,..]", help_heading = "Tag options")]
    remove: Vec<StringList>,

    /// Tag list to set (can be specified multiple times)
    #[clap(
        long,
        value_name = "TAG[,TAG,..]",
        conflicts_with = "remove",
        help_heading = "Tag options"
    )]
    set: Vec<StringList>,

    /// Remove any delete mark
    #[clap(
        long,
        conflicts_with_all = &["set_delete_never", "set_delete_after"], 
        help_heading = "Delete mark options"
    )]
    remove_delete: bool,

    /// Mark snapshot as uneraseable
    #[clap(
        long,
        conflicts_with = "set_delete_after",
        help_heading = "Delete mark options"
    )]
    set_delete_never: bool,

    /// Mark snapshot to be deleted after given duration (e.g. 10d)
    #[clap(long, value_name = "DURATION", help_heading = "Delete mark options")]
    set_delete_after: Option<humantime::Duration>,
}

impl Runnable for TagCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl TagCmd {
    fn inner_run(&self) -> anyhow::Result<()> {
        let config = RUSTIC_APP.config();
        let repo = open_repository(&config)?;

        let snapshots = if self.ids.is_empty() {
            repo.get_matching_snapshots(|sn| config.snapshot_filter.matches(sn))?
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
            (_, _, Some(d)) => Some(DeleteOption::After(Local::now() + Duration::from_std(*d)?)),
            (false, false, None) => None,
        };

        let snapshots: Vec<_> = snapshots
            .into_iter()
            .filter_map(|mut sn| {
                sn.modify_sn(self.set.clone(), self.add.clone(), &self.remove, &delete)
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
