//! `restore` subcommand

use std::io::IsTerminal;
use std::time::Instant;

use crate::{
    Application, RUSTIC_APP, helpers::bytes_size_to_string, repository::IndexedRepo, status_err,
};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::{Result, bail};
use dialoguer::Confirm;
use log::{debug, info};

use rustic_core::{LocalDestination, LsOptions, RestoreOptions};

use crate::filtering::SnapshotFilter;

/// `restore` subcommand
#[allow(clippy::struct_excessive_bools)]
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct RestoreCmd {
    /// Snapshot/path to restore
    ///
    /// Snapshot can be identified the following ways: "01a2b3c4" or "latest" or "latest~N" (N >= 0)
    #[clap(value_name = "SNAPSHOT[:PATH]")]
    snap: String,

    /// Restore destination
    #[clap(value_name = "DESTINATION")]
    dest: String,

    /// Restore options
    #[clap(flatten)]
    opts: RestoreOptions,

    /// List options
    #[clap(flatten)]
    ls_opts: LsOptions,

    /// Show interactive restore summary and confirmation prompt.
    /// Defaults to true when running in a terminal, false otherwise.
    #[clap(short = 'i', long = "interactive")]
    interactive: Option<bool>,

    /// Skip confirmation prompt (useful for scripting)
    #[clap(short = 'y', long = "yes")]
    skip_confirm: bool,

    /// Snapshot filter options (when using latest)
    #[clap(
        flatten,
        next_help_heading = "Snapshot filter options (when using latest)"
    )]
    filter: SnapshotFilter,
}

impl RestoreCmd {
    /// Determine whether to use interactive mode
    fn is_interactive(&self) -> bool {
        self.interactive
            .unwrap_or_else(|| std::io::stderr().is_terminal())
    }
}

impl Runnable for RestoreCmd {
    fn run(&self) {
        if let Err(err) = RUSTIC_APP
            .config()
            .repository
            .run_indexed(|repo| self.inner_run(repo))
        {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl RestoreCmd {
    fn inner_run(&self, repo: IndexedRepo) -> Result<()> {
        let config = RUSTIC_APP.config();
        let dry_run = config.global.dry_run;
        let interactive = self.is_interactive();

        // Validate snapshot identifier is not empty
        if self.snap.trim().is_empty() {
            bail!("Snapshot identifier cannot be empty. Use a snapshot ID (e.g. '01a2b3c4'), 'latest', or 'latest~N'.");
        }

        // Validate destination is not empty
        if self.dest.trim().is_empty() {
            bail!("Restore destination cannot be empty.");
        }

        let node =
            repo.node_from_snapshot_path(&self.snap, |sn| config.snapshot_filter.matches(sn))?;

        // for restore, always recurse into tree
        let mut ls_opts = self.ls_opts.clone();
        ls_opts.recursive = true;
        let ls = repo.ls(&node, &ls_opts)?;

        let dest = LocalDestination::new(&self.dest, true, !node.is_dir())?;

        let restore_infos = repo.prepare_restore(&self.opts, ls, &dest, dry_run)?;

        let fs = restore_infos.stats.files;
        let ds = restore_infos.stats.dirs;

        if interactive {
            // Rich restore summary panel
            println!();
            println!("┌─────────────────────── Restore Plan ───────────────────────┐");
            println!("│                                                            │");
            println!("│  Source:    {:<46} │", truncate_str(&self.snap, 46));
            println!("│  Target:   {:<46} │", truncate_str(&self.dest, 46));
            println!("│                                                            │");
            println!(
                "│  Files:    {:>6} to restore, {:>6} unchanged              │",
                fs.restore, fs.unchanged
            );
            println!(
                "│           {:>6} verified,   {:>6} to modify               │",
                fs.verified, fs.modify
            );
            if fs.additional > 0 {
                println!(
                    "│           {:>6} additional                                 │",
                    fs.additional
                );
            }
            println!(
                "│  Dirs:     {:>6} to restore, {:>6} to modify               │",
                ds.restore, ds.modify
            );
            if ds.additional > 0 {
                println!(
                    "│           {:>6} additional                                 │",
                    ds.additional
                );
            }
            println!("│                                                            │");
            println!(
                "│  Size:     {:<46} │",
                bytes_size_to_string(restore_infos.restore_size)
            );
            if restore_infos.matched_size > 0 {
                println!(
                    "│  Reusing:  {:<46} │",
                    format!(
                        "{} from existing files",
                        bytes_size_to_string(restore_infos.matched_size)
                    )
                );
            }
            println!("│                                                            │");
            println!("└────────────────────────────────────────────────────────────┘");
            println!();
        } else {
            // Standard non-interactive output (original behavior)
            println!(
                "Files:  {} to restore, {} unchanged, {} verified, {} to modify, {} additional",
                fs.restore, fs.unchanged, fs.verified, fs.modify, fs.additional
            );
            println!(
                "Dirs:   {} to restore, {} to modify, {} additional",
                ds.restore, ds.modify, ds.additional
            );

            info!(
                "total restore size: {}",
                bytes_size_to_string(restore_infos.restore_size)
            );
            if restore_infos.matched_size > 0 {
                info!(
                    "using {} of existing file contents.",
                    bytes_size_to_string(restore_infos.matched_size)
                );
            }
        }

        if restore_infos.restore_size == 0 {
            info!("all file contents are fine.");
        }

        // Confirmation prompt (unless --yes or non-interactive + piped)
        if !dry_run && !self.skip_confirm && interactive && restore_infos.restore_size > 0 {
            let proceed = Confirm::new()
                .with_prompt("Proceed with restore?")
                .default(true)
                .interact()?;
            if !proceed {
                println!("Restore cancelled.");
                return Ok(());
            }
        }

        if dry_run && config.global.dry_run_warmup {
            repo.warm_up(restore_infos.to_packs().into_iter())?;
        } else if !dry_run && !config.global.dry_run_warmup {
            let start = Instant::now();

            // save some memory
            let repo = repo.drop_data_from_index();

            let ls = repo.ls(&node, &ls_opts)?;
            repo.restore(restore_infos, &self.opts, ls, &dest)?;

            let elapsed = start.elapsed();
            let secs = elapsed.as_secs();
            let (hours, remainder) = (secs / 3600, secs % 3600);
            let (minutes, seconds) = (remainder / 60, remainder % 60);

            println!();
            if hours > 0 {
                println!(
                    "✓ Restore completed in {}h {}m {}s",
                    hours, minutes, seconds
                );
            } else if minutes > 0 {
                println!("✓ Restore completed in {}m {}s", minutes, seconds);
            } else {
                println!("✓ Restore completed in {:.1}s", elapsed.as_secs_f64());
            }
        } else {
            debug!(
                "--dry-run is without warmup, --dry-run --dry-run-warmup also issues the warmup script."
            );
        }

        Ok(())
    }
}

/// Truncate a string to a max length (by characters), adding "..." if truncated
fn truncate_str(s: &str, max_len: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_len {
        s.to_string()
    } else if max_len > 3 {
        format!("{}...", s.chars().take(max_len - 3).collect::<String>())
    } else {
        s.chars().take(max_len).collect::<String>()
    }
}
