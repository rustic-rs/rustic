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
    let snapshot: serde_json::Value = serde_json::from_slice(&backup_output.stdout)?;

    assert_eq!(snapshot["program_version"].as_str(), Some(version.trim()));

    Ok(())
}

#[cfg(unix)]
fn unreadable_backup_source() -> TestResult<Option<(TempDir, std::path::PathBuf)>> {
    use std::fs::{self, File, Permissions};
    use std::os::unix::fs::PermissionsExt;

    let src = tempdir()?;
    fs::write(src.path().join("ok.txt"), "ok")?;
    let secret = src.path().join("secret.txt");
    fs::write(&secret, "secret")?;
    fs::set_permissions(&secret, Permissions::from_mode(0o000))?;

    if File::open(&secret).is_ok() {
        fs::set_permissions(&secret, Permissions::from_mode(0o644))?;
        return Ok(None);
    }

    Ok(Some((src, secret)))
}

#[cfg(unix)]
#[test]
fn test_backup_unreadable_file_exits_3() -> TestResult<()> {
    use std::fs::{self, Permissions};
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = setup()?;
    let Some((src, secret)) = unreadable_backup_source()? else {
        return Ok(());
    };

    let result = rustic_runner(&temp_dir)?
        .arg("backup")
        .arg(src.path())
        .output()?;

    // Restore permissions so TempDir cleanup succeeds
    fs::set_permissions(&secret, Permissions::from_mode(0o644))?;

    assert_eq!(
        result.status.code(),
        Some(3),
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("at least one source file could not be read"),
        "stderr: {stderr}"
    );

    Ok(())
}

#[cfg(unix)]
#[test]
fn test_backup_unreadable_file_json_exit_error() -> TestResult<()> {
    use std::fs::{self, Permissions};
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = setup()?;
    let Some((src, secret)) = unreadable_backup_source()? else {
        return Ok(());
    };

    let password = "test";
    let repo_dir = temp_dir.path().join("repo");
    let result = Command::new(env!("CARGO_BIN_EXE_rustic"))
        .arg("-r")
        .arg(&repo_dir)
        .arg("--password")
        .arg(password)
        .arg("--json-progress")
        .arg("backup")
        .arg(src.path())
        .output()?;

    fs::set_permissions(&secret, Permissions::from_mode(0o644))?;

    assert_eq!(
        result.status.code(),
        Some(3),
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    let stderr = String::from_utf8_lossy(&result.stderr);
    let json_msgs: Vec<serde_json::Value> = stderr
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();

    assert!(
        json_msgs.iter().any(|v| v["message_type"] == "error"),
        "expected JSON error message in stderr: {stderr}"
    );
    let exit_error = json_msgs
        .iter()
        .find(|v| v["message_type"] == "exit_error")
        .expect("expected JSON exit_error in stderr");
    assert_eq!(exit_error["code"], 3);
    assert!(
        exit_error["message"]
            .as_str()
            .is_some_and(|m| m.contains("at least one source file could not be read"))
    );

    let stdout = String::from_utf8_lossy(&result.stdout);
    let stdout_msgs: Vec<serde_json::Value> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    assert!(
        stdout_msgs.iter().any(|v| v["message_type"] == "summary"),
        "expected JSON summary on stdout: {stdout}"
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
