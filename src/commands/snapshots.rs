//! `snapshot` subcommand

use crate::{
    Application, RUSTIC_APP,
    helpers::bytes_size_to_string,
    jinja::output_snapshots,
    repository::{OpenRepo, get_global_grouped_snapshots},
    status_err,
};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use derive_more::From;

use rustic_core::{ProgressBars, ProgressType, SnapshotGroup, repofile::SnapshotFile};
use serde::Serialize;

#[cfg(feature = "tui")]
use crate::commands::tui;

/// `snapshot` subcommand
#[derive(clap::Parser, Command, Debug, Serialize)]
pub(crate) struct SnapshotCmd {
    /// Snapshots to show. If none is given, use filter options to filter from all snapshots
    ///
    /// Snapshots can be identified the following ways: "01a2b3c4" or "latest" or "latest~N" (N >= 0)
    #[clap(value_name = "ID")]
    ids: Vec<String>,

    /// Show detailed information about snapshots
    #[arg(long)]
    long: bool,

    /// Show snapshots in json format
    #[clap(long, conflicts_with = "long")]
    json: bool,

    /// Show all snapshots instead of summarizing identical follow-up snapshots
    #[clap(long, conflicts_with_all = &["long", "json"])]
    all: bool,

    #[cfg(feature = "tui")]
    /// Run in interactive UI mode
    #[clap(long, short)]
    pub interactive: bool,
}

impl Runnable for SnapshotCmd {
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

impl SnapshotCmd {
    fn inner_run(&self, repo: OpenRepo) -> Result<()> {
        #[cfg(feature = "tui")]
        if self.interactive {
            return tui::run(|progress| {
                let config = RUSTIC_APP.config();
                config
                    .repository
                    .run_indexed_with_progress(progress.clone(), |repo| {
                        let p = progress.progress(
                            ProgressType::Spinner,
                            "starting rustic in interactive mode...",
                        );
                        p.finish();
                        // create app and run it
                        let snapshots = tui::Snapshots::new(
                            &repo,
                            config.snapshot_filter.clone(),
                            config.global.group_by.unwrap_or_default(),
                        )?;
                        tui::run_app(progress.terminal, snapshots)
                    })
            });
        }

        let groups = get_global_grouped_snapshots(&repo, &self.ids)?.groups;

        if self.json {
            let mut stdout = std::io::stdout();
            if groups.len() == 1 && groups[0].group_key.is_empty() {
                // we don't use grouping, only output snapshots list
                serde_json::to_writer_pretty(&mut stdout, &groups[0].items)?;
            } else {
                #[derive(Serialize, From)]
                struct SnapshotsGroup {
                    group_key: SnapshotGroup,
                    snapshots: Vec<SnapshotFile>,
                }
                let groups: Vec<SnapshotsGroup> = groups
                    .into_iter()
                    .map(|g| (g.group_key, g.items).into())
                    .collect();
                serde_json::to_writer_pretty(&mut stdout, &groups)?;
            }
            return Ok(());
        }

        let output = output_snapshots(&groups, self)?;
        println!("{output}");

        Ok(())
    }
}

pub fn snap_to_table(sn: &SnapshotFile, count: usize) -> [String; 9] {
    let config = RUSTIC_APP.config();
    let tags = sn.tags.formatln();
    let paths = sn.paths.formatln();
    let time = config.global.format_time(&sn.time);
    let (files, dirs, size) = sn.summary.as_ref().map_or_else(
        || ("?".to_string(), "?".to_string(), "?".to_string()),
        |s| {
            (
                s.total_files_processed.to_string(),
                s.total_dirs_processed.to_string(),
                bytes_size_to_string(s.total_bytes_processed),
            )
        },
    );
    let id = match count {
        0 => format!("{}", sn.id),
        count => format!("{} (+{})", sn.id, count),
    };
    [
        id,
        time.to_string(),
        sn.hostname.clone(),
        sn.label.clone(),
        tags,
        paths,
        files,
        dirs,
        size,
    ]
}
