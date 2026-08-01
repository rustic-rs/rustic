//! Rustic Integration Test for Backups and Restore
//!
//! Runs the application as a subprocess and asserts its
//! output for the `init`, `backup`, `restore`, `check`,
//! and `snapshots` command
//!
//! You can run them with 'nextest':
//! `cargo nextest run -E 'test(backup)'`.

#[cfg(unix)]
use std::os::unix::fs::symlink;

use dircmp::Comparison;
use tempfile::{TempDir, tempdir};

use assert_cmd::Command;
use predicates::prelude::{PredicateBooleanExt, predicate};

mod repositories;
use repositories::src_snapshot;

use rustic_testing::TestResult;

pub fn rustic_runner(temp_dir: &TempDir) -> TestResult<Command> {
    let password = "test";
    let repo_dir = temp_dir.path().join("repo");

    let mut runner = Command::new(env!("CARGO_BIN_EXE_rustic"));

    runner
        .arg("-r")
        .arg(repo_dir)
        .arg("--password")
        .arg(password)
        .arg("--no-progress");

    Ok(runner)
}

fn rustic_runner_with_json_progress(temp_dir: &TempDir) -> TestResult<Command> {
    let password = "test";
    let repo_dir = temp_dir.path().join("repo");

    let mut runner = Command::new(env!("CARGO_BIN_EXE_rustic"));

    runner
        .arg("-r")
        .arg(repo_dir)
        .arg("--password")
        .arg(password)
        .arg("--json-progress")
        .args(["--progress-interval", "1ms"]);

    Ok(runner)
}

fn setup() -> TestResult<TempDir> {
    let temp_dir = tempdir()?;
    rustic_runner(&temp_dir)?
        .args(["init"])
        .assert()
        .success()
        .stderr(predicate::str::contains("successfully created."))
        .stderr(predicate::str::contains("successfully added."));

    Ok(temp_dir)
}

#[test]
fn test_backup_and_check_passes() -> TestResult<()> {
    let temp_dir = setup()?;
    let backup = src_snapshot()?.into_path();

    {
        // Run `backup` for the first time
        rustic_runner(&temp_dir)?
            .arg("backup")
            .arg(backup.path())
            .assert()
            .success()
            .stderr(predicate::str::contains("successfully saved."));
    }

    {
        // Run `snapshots`
        rustic_runner(&temp_dir)?
            .arg("snapshots")
            .assert()
            .success()
            .stdout(predicate::str::contains("total: 1 snapshot(s)"));
    }

    {
        // Run `backup` a second time
        rustic_runner(&temp_dir)?
            .arg("backup")
            .arg(backup.path())
            .assert()
            .success()
            .stderr(predicate::str::contains("Added to the repo: 0 B"))
            .stderr(predicate::str::contains("successfully saved."));
    }

    {
        // Run `snapshots` a second time
        rustic_runner(&temp_dir)?
            .arg("snapshots")
            .assert()
            .success()
            .stdout(predicate::str::contains("total: 2 snapshot(s)"));
    }

    {
        // Run `check --read-data`
        rustic_runner(&temp_dir)?
            .args(["check", "--read-data"])
            .assert()
            .success()
            .stderr(predicate::str::contains("WARN").not())
            .stderr(predicate::str::contains("ERROR").not());
    }

    Ok(())
}

#[test]
fn snapshots_can_select_tabular_columns() -> TestResult<()> {
    let temp_dir = setup()?;
    let source = temp_dir.path().join("source");
    std::fs::create_dir(&source)?;
    std::fs::write(source.join("payload.txt"), "payload")?;

    rustic_runner(&temp_dir)?
        .arg("backup")
        .arg(&source)
        .assert()
        .success();

    rustic_runner(&temp_dir)?
        .args(["snapshots", "--columns", "id,host,paths"])
        .assert()
        .success()
        .stdout(predicate::str::contains("| ID"))
        .stdout(predicate::str::contains("| Host"))
        .stdout(predicate::str::contains("| Paths"))
        .stdout(predicate::str::contains("| Time").not())
        .stdout(predicate::str::contains("| Size").not());

    Ok(())
}

#[test]
fn include_if_present_backs_up_only_marked_directory_trees() -> TestResult<()> {
    let temp_dir = setup()?;
    let source = temp_dir.path().join("source");
    let ignored = source.join("ignored");
    let included = source.join("included");
    std::fs::create_dir_all(&ignored)?;
    std::fs::create_dir_all(&included)?;
    std::fs::write(ignored.join("not-backed-up.txt"), "ignore me")?;
    std::fs::write(included.join(".bckinclude"), "")?;
    std::fs::write(included.join("backed-up.txt"), "include me")?;

    let output = rustic_runner(&temp_dir)?
        .args(["backup", "--include-if-present", ".bckinclude", "--json"])
        .arg(&source)
        .output()?;
    assert!(
        output.status.success(),
        "marked backup failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let snapshot: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let paths = snapshot["paths"]
        .as_array()
        .expect("backup JSON must include snapshot paths");
    let included = included.canonicalize()?.to_string_lossy().into_owned();
    assert_eq!(paths.len(), 1, "unexpected marked sources: {paths:?}");
    assert_eq!(paths[0].as_str(), Some(included.as_str()));

    Ok(())
}

#[test]
fn restore_json_progress_writes_only_newline_delimited_json_to_stdout() -> TestResult<()> {
    let temp_dir = setup()?;
    let source = temp_dir.path().join("source");
    std::fs::create_dir(&source)?;
    std::fs::write(source.join("payload.bin"), vec![0_u8; 256 * 1024])?;

    rustic_runner(&temp_dir)?
        .arg("backup")
        .arg(&source)
        .assert()
        .success();

    let output = rustic_runner_with_json_progress(&temp_dir)?
        .args(["restore", "latest"])
        .arg(temp_dir.path().join("restore"))
        .output()?;

    assert!(
        output.status.success(),
        "restore command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout)?;
    let events = stdout
        .lines()
        .filter(|line| !line.is_empty())
        .map(serde_json::from_str::<serde_json::Value>)
        .collect::<Result<Vec<_>, _>>()?;

    assert!(
        !events.is_empty(),
        "restore --json-progress did not produce progress events"
    );
    assert!(
        events.iter().all(|event| event["message_type"] == "status"),
        "unexpected restore JSON progress output: {stdout}"
    );

    Ok(())
}

#[test]
fn configured_snapshots_write_to_their_own_log_files() -> TestResult<()> {
    let temp_dir = setup()?;
    let first_source = temp_dir.path().join("first-source");
    let second_source = temp_dir.path().join("second-source");
    std::fs::create_dir(&first_source)?;
    std::fs::create_dir(&second_source)?;
    std::fs::write(first_source.join("first.txt"), "first")?;
    std::fs::write(second_source.join("second.txt"), "second")?;

    let first_log = temp_dir.path().join("first.log");
    let second_log = temp_dir.path().join("second.log");
    let profile = temp_dir.path().join("snapshot-logs.toml");
    std::fs::write(
        &profile,
        format!(
            r#"
                [[backup.snapshots]]
                sources = ["{}"]
                log-file = "{}"

                [[backup.snapshots]]
                sources = ["{}"]
                log-file = "{}"
            "#,
            first_source.display(),
            first_log.display(),
            second_source.display(),
            second_log.display(),
        ),
    )?;

    rustic_runner(&temp_dir)?
        .args(["-P", profile.to_str().unwrap(), "backup"])
        .assert()
        .success();

    let first_log = std::fs::read_to_string(&first_log)?;
    let second_log = std::fs::read_to_string(&second_log)?;
    let first_source = first_source.display().to_string();
    let second_source = second_source.display().to_string();

    assert!(first_log.contains(&format!("backup of {first_source} done.")));
    assert!(!first_log.contains(&format!("backup of {second_source} done.")));
    assert!(second_log.contains(&format!("backup of {second_source} done.")));
    assert!(!second_log.contains(&format!("backup of {first_source} done.")));

    Ok(())
}

#[test]
fn backup_filter_selects_configured_snapshots_by_tag_and_path() -> TestResult<()> {
    let temp_dir = setup()?;
    let local_source = temp_dir.path().join("local-source");
    let external_source = temp_dir.path().join("external-source");
    std::fs::create_dir(&local_source)?;
    std::fs::create_dir(&external_source)?;
    std::fs::write(local_source.join("local.txt"), "local")?;
    std::fs::write(external_source.join("external.txt"), "external")?;

    let profile = temp_dir.path().join("snapshot-filters.toml");
    let local_source_toml = toml::Value::String(local_source.to_string_lossy().into_owned());
    let external_source_toml = toml::Value::String(external_source.to_string_lossy().into_owned());
    std::fs::write(
        &profile,
        format!(
            r#"
                [[backup.snapshots]]
                sources = [{local_source_toml}]
                tags = ["local"]

                [[backup.snapshots]]
                sources = [{external_source_toml}]
                tags = ["external"]
            "#,
        ),
    )?;

    let tagged = rustic_runner(&temp_dir)?
        .arg("-P")
        .arg(&profile)
        .args(["--filter-tags", "local", "backup", "--json"])
        .output()?;
    assert!(
        tagged.status.success(),
        "tag-filtered backup failed: {}",
        String::from_utf8_lossy(&tagged.stderr)
    );
    let tagged_snapshot: serde_json::Value = serde_json::from_slice(&tagged.stdout)?;
    assert_eq!(tagged_snapshot["tags"], serde_json::json!(["local"]));

    let path_filtered = rustic_runner(&temp_dir)?
        .arg("-P")
        .arg(&profile)
        .arg("--filter-paths")
        .arg(&external_source)
        .args(["backup", "--json"])
        .output()?;
    assert!(
        path_filtered.status.success(),
        "path-filtered backup failed: {}",
        String::from_utf8_lossy(&path_filtered.stderr)
    );
    let path_filtered_snapshot: serde_json::Value = serde_json::from_slice(&path_filtered.stdout)?;
    assert_eq!(
        path_filtered_snapshot["tags"],
        serde_json::json!(["external"])
    );

    Ok(())
}

#[cfg(unix)]
#[test]
fn diff_reports_unchanged_symlinks_as_identical() -> TestResult<()> {
    let temp_dir = setup()?;
    let source = temp_dir.path().join("source");
    std::fs::create_dir(&source)?;
    std::fs::write(source.join("target.txt"), "target")?;
    symlink("target.txt", source.join("link.txt"))?;

    rustic_runner(&temp_dir)?
        .arg("backup")
        .arg(&source)
        .assert()
        .success();

    std::fs::write(source.join("added.txt"), "added")?;

    rustic_runner(&temp_dir)?
        .arg("backup")
        .arg(&source)
        .assert()
        .success();

    let output = rustic_runner(&temp_dir)?
        .args(["diff", "latest~1", "latest"])
        .output()?;

    assert!(
        output.status.success(),
        "diff command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout)?;
    assert!(
        stdout.contains("Symlinks: 1 =, 0 +, 0 -, 0 M, 0 U"),
        "unexpected diff output: {stdout}"
    );
    assert!(
        !stdout.contains("link.txt"),
        "unchanged symlink was reported as changed: {stdout}"
    );

    Ok(())
}

#[test]
fn test_backup_records_cli_version_in_snapshot() -> TestResult<()> {
    let temp_dir = setup()?;
    let backup = src_snapshot()?.into_path();

    let version_output = Command::new(env!("CARGO_BIN_EXE_rustic"))
        .arg("--version")
        .output()?;
    assert!(version_output.status.success());
    let version = String::from_utf8(version_output.stdout)?;

    let backup_output = rustic_runner(&temp_dir)?
        .args(["backup", "--json"])
        .arg(backup.path())
        .output()?;
    assert!(backup_output.status.success());
    assert!(
        !backup_output.stdout.contains(&b'\n'),
        "--json output must be a single line"
    );
    let snapshot: serde_json::Value = serde_json::from_slice(&backup_output.stdout)?;

    assert_eq!(snapshot["program_version"].as_str(), Some(version.trim()));

    Ok(())
}

#[test]
fn copy_target_profile_substitutes_environment_options() -> TestResult<()> {
    let source = setup()?;
    let target = setup()?;
    let target_profile = source.path().join("target.toml");

    std::fs::write(
        &target_profile,
        r#"
            [repository]
            repository = "${COPY_TARGET_REPOSITORY}"
            password = "${COPY_TARGET_PASSWORD}"
        "#,
    )?;

    let output = rustic_runner(&source)?
        .arg("--profile-substitute-env")
        .args(["copy", "--target"])
        .arg(&target_profile)
        .arg("--dry-run")
        .env("COPY_TARGET_REPOSITORY", target.path().join("repo"))
        .env("COPY_TARGET_PASSWORD", "test")
        .output()?;

    assert!(
        output.status.success(),
        "copy command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8(output.stderr)?.contains("copying to target"),
        "target profile was not opened successfully"
    );

    Ok(())
}

#[test]
fn backup_accepts_an_empty_as_path() -> TestResult<()> {
    let temp_dir = setup()?;
    let source = temp_dir.path().join("source");
    std::fs::create_dir(&source)?;
    std::fs::write(source.join("payload.txt"), "payload")?;

    rustic_runner(&temp_dir)?
        .args(["backup", "--as-path", ""])
        .arg(&source)
        .assert()
        .success()
        .stderr(predicate::str::contains("successfully saved."));

    Ok(())
}

#[test]
fn one_file_system_keeps_explicit_nested_backup_sources() -> TestResult<()> {
    let temp_dir = setup()?;
    let source = temp_dir.path().join("source");
    let nested_source = source.join("separate-mount");
    std::fs::create_dir_all(&nested_source)?;
    std::fs::write(source.join("root.txt"), "root")?;
    std::fs::write(nested_source.join("nested.txt"), "nested")?;

    let output = rustic_runner(&temp_dir)?
        .args(["backup", "--one-file-system", "--json"])
        .arg(&source)
        .arg(&nested_source)
        .output()?;

    assert!(
        output.status.success(),
        "backup command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let snapshot: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let paths = snapshot["paths"]
        .as_array()
        .expect("backup JSON must include snapshot paths");
    let source = source.canonicalize()?.to_string_lossy().into_owned();
    let nested_source = nested_source.canonicalize()?.to_string_lossy().into_owned();

    assert!(
        paths.iter().any(|path| path.as_str() == Some(&source)),
        "parent source is missing from snapshot paths: {paths:?}"
    );
    assert!(
        paths
            .iter()
            .any(|path| path.as_str() == Some(&nested_source)),
        "explicit nested source is missing from snapshot paths: {paths:?}"
    );

    Ok(())
}

#[test]
fn test_backup_and_restore_passes() -> TestResult<()> {
    let temp_dir = setup()?;
    let restore_dir = temp_dir.path().join("restore");
    let backup_files = src_snapshot()?.into_path();

    {
        // Run `backup` for the first time
        rustic_runner(&temp_dir)?
            .arg("backup")
            .arg(backup_files.path())
            .arg("--as-path")
            .arg("/")
            .assert()
            .success()
            .stderr(predicate::str::contains("successfully saved."));
    }
    {
        // Run `restore`
        rustic_runner(&temp_dir)?
            .arg("restore")
            .arg("latest")
            .arg(&restore_dir)
            .assert()
            .success()
            .stdout(predicate::str::contains("restore done"));
    }

    // Compare the backup and the restored directory
    let compare_result = Comparison::default().compare(backup_files.path(), &restore_dir)?;

    // no differences
    assert!(compare_result.is_empty());

    let dump_tar_file = restore_dir.join("test.tar");
    {
        // Run `dump`
        rustic_runner(&temp_dir)?
            .arg("dump")
            .arg("latest")
            .arg("--file")
            .arg(&dump_tar_file)
            .assert()
            .success();
    }
    // TODO: compare dump output with fixture

    Ok(())
}
