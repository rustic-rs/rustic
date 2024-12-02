//! Rustic Integration Test for Backups and Restore
//!
//! Runs the application as a subprocess and asserts its
//! output for the `init`, `backup`, `restore`, `check`,
//! and `snapshots` command
//!
//! You can run them with 'nextest':
//! `cargo nextest run -E 'test(backup)'`.

use dircmp::Comparison;
use tempfile::{tempdir, TempDir};

use assert_cmd::Command;
use predicates::prelude::{predicate, PredicateBooleanExt};

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
    let backup = src_snapshot()?.into_path().into_path();

    {
        // Run `backup` for the first time
        rustic_runner(&temp_dir)?
            .arg("backup")
            .arg(&backup)
            .assert()
            .success()
            .stdout(predicate::str::contains("successfully saved."));
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
            .arg(backup)
            .assert()
            .success()
            .stdout(predicate::str::contains("Added to the repo: 0 B"))
            .stdout(predicate::str::contains("successfully saved."));
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
fn test_backup_and_restore_passes() -> TestResult<()> {
    let temp_dir = setup()?;
    let restore_dir = temp_dir.path().join("restore");
    let backup_files = src_snapshot()?.into_path().into_path();

    {
        // Run `backup` for the first time
        rustic_runner(&temp_dir)?
            .arg("backup")
            .arg(&backup_files)
            .arg("--as-path")
            .arg("/")
            .assert()
            .success()
            .stdout(predicate::str::contains("successfully saved."));
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
    let compare_result = Comparison::default().compare(&backup_files, &restore_dir)?;

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
