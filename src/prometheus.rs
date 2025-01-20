use std::{env, error::Error};

use prometheus::register_gauge;
use rustic_core::repofile::SnapshotFile;

pub fn parse_label(
    s: &str,
) -> Result<(String, String), Box<dyn Error + Send + Sync + 'static>> {
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid prometheus label definition: no `=` found in `{s}`"))?;
    Ok((s[..pos].to_owned(), s[pos + 1..].to_owned()))
}

pub fn publish_metrics(
    pushgateway_url: &str,
    job_name: &str,
    snap: &SnapshotFile,
    user_labels: Vec<(String, String)>,
) -> anyhow::Result<()> {
    let summary = snap.summary.as_ref().expect("Reaching the 'push to prometheus' point should only happen for successful backups, which must have a summary set.");

    let metric_time = register_gauge!("rustic_backup_time", "Timestamp of this snapshot",).unwrap();
    metric_time.set(snap.time.timestamp_millis() as f64 / 1000.);
    let metric_files_new = register_gauge!(
        "rustic_backup_files_new",
        "New files compared to the last (i.e. parent) snapshot",
    )
    .unwrap();
    metric_files_new.set(summary.files_new as f64);
    let metric_files_changed = register_gauge!(
        "rustic_backup_files_changed",
        "Changed files compared to the last (i.e. parent) snapshot",
    )
    .unwrap();
    metric_files_changed.set(summary.files_changed as f64);
    let metric_files_unmodified = register_gauge!(
        "rustic_backup_files_unmodified",
        "Unchanged files compared to the last (i.e. parent) snapshot",
    )
    .unwrap();
    metric_files_unmodified.set(summary.files_unmodified as f64);
    let metric_total_files_processed = register_gauge!(
        "rustic_backup_total_files_processed",
        "Total processed files",
    )
    .unwrap();
    metric_total_files_processed.set(summary.total_files_processed as f64);
    let metric_total_bytes_processed = register_gauge!(
        "rustic_backup_total_bytes_processed",
        "Total size of all processed files",
    )
    .unwrap();
    metric_total_bytes_processed.set(summary.total_bytes_processed as f64);
    let metric_dirs_new = register_gauge!(
        "rustic_backup_dirs_new",
        "New directories compared to the last (i.e. parent) snapshot",
    )
    .unwrap();
    metric_dirs_new.set(summary.dirs_new as f64);
    let metric_dirs_changed = register_gauge!(
        "rustic_backup_dirs_changed",
        "Changed directories compared to the last (i.e. parent) snapshot",
    )
    .unwrap();
    metric_dirs_changed.set(summary.dirs_changed as f64);
    let metric_dirs_unmodified = register_gauge!(
        "rustic_backup_dirs_unmodified",
        "Unchanged directories compared to the last (i.e. parent) snapshot",
    )
    .unwrap();
    metric_dirs_unmodified.set(summary.dirs_unmodified as f64);
    let metric_total_dirs_processed = register_gauge!(
        "rustic_backup_total_dirs_processed",
        "Total processed directories",
    )
    .unwrap();
    metric_total_dirs_processed.set(summary.total_dirs_processed as f64);
    let metric_total_dirsize_processed = register_gauge!(
        "rustic_backup_total_dirsize_processed",
        "Total number of data blobs added by this snapshot",
    )
    .unwrap();
    metric_total_dirsize_processed.set(summary.total_dirsize_processed as f64);
    let metric_data_blobs = register_gauge!(
        "rustic_backup_data_blobs",
        "Total size of all processed dirs",
    )
    .unwrap();
    metric_data_blobs.set(summary.data_blobs as f64);
    let metric_tree_blobs = register_gauge!(
        "rustic_backup_tree_blobs",
        "Total number of tree blobs added by this snapshot",
    )
    .unwrap();
    metric_tree_blobs.set(summary.tree_blobs as f64);
    let metric_data_added = register_gauge!(
        "rustic_backup_data_added",
        "Total uncompressed bytes added by this snapshot",
    )
    .unwrap();
    metric_data_added.set(summary.data_added as f64);
    let metric_data_added_packed = register_gauge!(
        "rustic_backup_data_added_packed",
        "Total bytes added to the repository by this snapshot",
    )
    .unwrap();
    metric_data_added_packed.set(summary.data_added_packed as f64);
    let metric_data_added_files = register_gauge!(
        "rustic_backup_data_added_files",
        "Total uncompressed bytes (new/changed files) added by this snapshot",
    )
    .unwrap();
    metric_data_added_files.set(summary.data_added_files as f64);
    let metric_data_added_files_packed = register_gauge!(
        "rustic_backup_data_added_files_packed",
        "Total bytes for new/changed files added to the repository by this snapshot",
    )
    .unwrap();
    metric_data_added_files_packed.set(summary.data_added_files_packed as f64);
    let metric_data_added_trees = register_gauge!(
        "rustic_backup_data_added_trees",
        "Total uncompressed bytes (new/changed directories) added by this snapshot",
    )
    .unwrap();
    metric_data_added_trees.set(summary.data_added_trees as f64);
    let metric_data_added_trees_packed = register_gauge!(
        "rustic_backup_data_added_trees_packed",
        "Total bytes (new/changed directories) added to the repository by this snapshot",
    )
    .unwrap();
    metric_data_added_trees_packed.set(summary.data_added_trees_packed as f64);
    let metric_backup_start = register_gauge!(
        "rustic_backup_backup_start",
        "Start time of the backup. This may differ from the snapshot `time`.",
    )
    .unwrap();
    metric_backup_start.set(summary.backup_start.timestamp_millis() as f64 / 1000.);
    let metric_backup_end = register_gauge!(
        "rustic_backup_backup_end",
        "The time that the backup has been finished.",
    )
    .unwrap();
    metric_backup_end.set(summary.backup_end.timestamp_millis() as f64 / 1000.);
    let metric_backup_duration = register_gauge!(
        "rustic_backup_backup_duration",
        "Total duration of the backup in seconds, i.e. the time between `backup_start` and `backup_end`",
    ).unwrap();
    metric_backup_duration.set(summary.backup_duration);
    let metric_total_duration = register_gauge!(
        "rustic_backup_total_duration",
        "Total duration that the rustic command ran in seconds",
    )
    .unwrap();
    metric_total_duration.set(summary.total_duration);

    let metric_families = prometheus::gather();

    let auth = match (env::var("PUSHGATEWAY_USER"), env::var("PUSHGATEWAY_PASS")) {
        (Ok(username), Ok(password)) => {
            Some(prometheus::BasicAuthentication { username, password })
        }
        (Err(env::VarError::NotPresent), Err(env::VarError::NotPresent)) => None,
        _ => panic!("Got partial or invalid pushgateway credentials"),
    };

    let mut labels = prometheus::labels! {
        "paths".to_owned() => format!("{}", snap.paths),
        "hostname".to_owned() => snap.hostname.clone(),
        "uid".to_owned() => format!("{}", snap.uid),
        "gid".to_owned() => format!("{}", snap.gid),
    };
    // See https://github.com/tikv/rust-prometheus/issues/535
    if !snap.label.is_empty() {
        let _ = labels.insert("snapshot_label".to_owned(), snap.label.clone());
    }
    if !snap.username.is_empty() {
        let _ = labels.insert("username".to_owned(), snap.username.clone());
    }
    let tags = format!("{}", snap.tags);
    if !tags.is_empty() {
        let _ = labels.insert("tags".to_owned(), tags);
    }
    labels.extend(user_labels);

    prometheus::push_metrics(job_name, labels, &pushgateway_url, metric_families, auth)?;

    Ok(())
}
