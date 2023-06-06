//! `smapshot` subcommand

/// App-local prelude includes `app_reader()`/`app_writer()`/`app_config()`
/// accessors along with logging macros. Customize as you see fit.
use crate::{
    commands::{get_repository, open_repository},
    status_err, Application, RUSTIC_APP,
};

use abscissa_core::{Command, Runnable, Shutdown};

use itertools::Itertools;

use rustic_core::helpers::table_output::table_right_from;
use rustic_core::{bytes_size_to_string, SnapshotFile, SnapshotGroup, SnapshotGroupCriterion};

/// `snapshot` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct SnapshotCmd {
    /// Snapshots to show. If none is given, use filter options to filter from all snapshots
    #[clap(value_name = "ID")]
    ids: Vec<String>,

    /// Group snapshots by any combination of host,label,paths,tags
    #[clap(
        long,
        short = 'g',
        value_name = "CRITERION",
        default_value = "host,label,paths"
    )]
    group_by: SnapshotGroupCriterion,

    /// Show detailed information about snapshots
    #[arg(long)]
    long: bool,

    /// Show snapshots in json format
    #[clap(long, conflicts_with = "long")]
    json: bool,

    /// Show all snapshots instead of summarizing identical follow-up snapshots
    #[clap(long, conflicts_with_all = &["long", "json"])]
    all: bool,
}
impl Runnable for SnapshotCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl SnapshotCmd {
    fn inner_run(&self) -> anyhow::Result<()> {
        let config = RUSTIC_APP.config();

        let repo = open_repository(get_repository(&config));

        let groups = match &self.ids[..] {
            [] => SnapshotFile::group_from_backend(
                &repo.dbe,
                |sn| config.snapshot_filter.matches(sn),
                &self.group_by,
            )?,
            [id] if id == "latest" => SnapshotFile::group_from_backend(
                &repo.dbe,
                |sn| config.snapshot_filter.matches(sn),
                &self.group_by,
            )?
            .into_iter()
            .map(|(group, mut snaps)| {
                snaps.sort_unstable();
                let last_idx = snaps.len() - 1;
                snaps.swap(0, last_idx);
                snaps.truncate(1);
                (group, snaps)
            })
            .collect::<Vec<_>>(),
            _ => {
                let item = (
                    SnapshotGroup::default(),
                    SnapshotFile::from_ids(&repo.dbe, &self.ids)?,
                );
                vec![item]
            }
        };

        if self.json {
            let mut stdout = std::io::stdout();
            serde_json::to_writer_pretty(&mut stdout, &groups)?;
            return Ok(());
        }

        for (group, mut snapshots) in groups {
            if !group.is_empty() {
                println!("\nsnapshots for {group}");
            }
            snapshots.sort_unstable();
            let count = snapshots.len();

            if self.long {
                for snap in snapshots {
                    println!("{snap}");
                    println!();
                }
            } else {
                let snap_to_table = |(sn, count): (SnapshotFile, usize)| {
                    let tags = sn.tags.formatln();
                    let paths = sn.paths.formatln();
                    let time = sn.time.format("%Y-%m-%d %H:%M:%S");
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
                        sn.hostname,
                        sn.label,
                        tags,
                        paths,
                        files,
                        dirs,
                        size,
                    ]
                };

                let mut table = table_right_from(
                    6,
                    [
                        "ID", "Time", "Host", "Label", "Tags", "Paths", "Files", "Dirs", "Size",
                    ],
                );

                let snapshots: Vec<_> = snapshots
                    .into_iter()
                    .group_by(|sn| if self.all { sn.id } else { sn.tree })
                    .into_iter()
                    .map(|(_, mut g)| (g.next().unwrap(), g.count()))
                    .map(snap_to_table)
                    .collect();
                _ = table.add_rows(snapshots);
                println!("{table}");
            }
            println!("{count} snapshot(s)");
        }

        Ok(())
    }
}
