//! Rustic Integration Test for Backups and Restore
//!
//! Runs the application as a subprocess and asserts its
//! output for the `init`, `backup`, `restore`, `check`,
//! and `snapshots` command
//!
//! You can run them with 'nextest':
//! `cargo nextest run -E 'test(backup)'`.

use abscissa_core::testing::prelude::*;

use aho_corasick::PatternID;
use dircmp::Comparison;
use pretty_assertions::assert_eq;
use rustic_testing::{get_matches, TestResult};
use std::io::Read;
use tempfile::{tempdir, TempDir};

pub fn rustic_runner(temp_dir: &TempDir) -> CmdRunner {
    let password = "test";
    let repo_dir = temp_dir.path().join("repo");
    let mut runner = CmdRunner::new(env!("CARGO_BIN_EXE_rustic"));
    runner
        .arg("-r")
        .arg(repo_dir)
        .arg("--password")
        .arg(password)
        .arg("--no-progress")
        .capture_stdout();
    runner
}

fn setup() -> TestResult<TempDir> {
    let temp_dir = tempdir()?;
    let mut runner = rustic_runner(&temp_dir);
    let mut cmd = runner.args(["init"]).run();

    let mut output = String::new();
    cmd.stdout().read_to_string(&mut output)?;

    let patterns = &["successfully added.", "successfully created."];
    let matches = get_matches(patterns, output)?;

    assert_eq!(
        matches,
        vec![(PatternID::must(0), 19), (PatternID::must(1), 21),]
    );

    cmd.wait()?.expect_success();
    Ok(temp_dir)
}

#[test]
fn test_backup_and_check_passes() -> TestResult<()> {
    let temp_dir = setup()?;
    let backup = "crates/";

    {
        // Run `backup` for the first time
        let mut runner = rustic_runner(&temp_dir);
        let mut cmd = runner.arg("backup").arg(backup).run();

        let mut output = String::new();
        cmd.stdout().read_to_string(&mut output)?;

        let patterns = &["successfully saved."];
        let matches = get_matches(patterns, output)?;

        assert_eq!(matches, vec![(PatternID::must(0), 19)]);
        cmd.wait()?.expect_success();
    }

    {
        // Run `snapshots`
        let mut runner = rustic_runner(&temp_dir);
        let mut cmd = runner.arg("snapshots").run();
        let mut output = String::new();
        cmd.stdout().read_to_string(&mut output)?;

        let patterns = &["1 snapshot(s)"];
        let matches = get_matches(patterns, output)?;

        assert_eq!(matches, vec![(PatternID::must(0), 13)]);

        cmd.wait()?.expect_success();
    }

    {
        // Run `backup` a second time
        let mut runner = rustic_runner(&temp_dir);
        let mut cmd = runner.arg("backup").arg(backup).run();

        let mut output = String::new();
        cmd.stdout().read_to_string(&mut output)?;

        let patterns = &["Added to the repo: 0 B", "successfully saved."];
        let matches = get_matches(patterns, output)?;

        assert_eq!(
            matches,
            vec![(PatternID::must(0), 22), (PatternID::must(1), 19)]
        );

        cmd.wait()?.expect_success();
    }

    {
        // Run `snapshots` a second time
        let mut runner = rustic_runner(&temp_dir);
        let mut cmd = runner.arg("snapshots").run();
        let mut output = String::new();
        cmd.stdout().read_to_string(&mut output)?;

        let patterns = &["2 snapshot(s)"];
        let matches = get_matches(patterns, output)?;

        assert_eq!(matches, vec![(PatternID::must(0), 13)]);

        cmd.wait()?.expect_success();
    }

    {
        // Run `check --read-data`
        let mut runner = rustic_runner(&temp_dir);
        let mut cmd = runner.args(["check", "--read-data"]).run();
        let mut output = String::new();
        cmd.stdout().read_to_string(&mut output)?;

        let patterns = &["WARN", "ERROR"];
        let matches = get_matches(patterns, output)?;

        assert_eq!(matches.len(), 0);

        cmd.wait()?.expect_success();
    }

    Ok(())
}

#[test]
fn test_backup_and_restore_passes() -> TestResult<()> {
    let temp_dir = setup()?;
    let restore_dir = temp_dir.path().join("restore");
    let backup = "crates";

    // actual repository root to backup
    let current_dir = std::env::current_dir()?;
    let backup_files = current_dir.join(backup);

    {
        // Run `backup` for the first time
        let mut runner = rustic_runner(&temp_dir);
        let mut cmd = runner.arg("backup").arg(&backup_files).run();

        let mut output = String::new();
        cmd.stdout().read_to_string(&mut output)?;

        let patterns = &["successfully saved."];
        let matches = get_matches(patterns, output)?;

        assert_eq!(matches, vec![(PatternID::must(0), 19)]);
        cmd.wait()?.expect_success();
    }
    {
        // Run `restore`
        let mut runner = rustic_runner(&temp_dir);
        let mut cmd = runner.arg("restore").arg("latest").arg(&restore_dir).run();

        let mut output = String::new();
        cmd.stdout().read_to_string(&mut output)?;

        let patterns = &["restore done"];
        let matches = get_matches(patterns, output)?;

        assert_eq!(matches, vec![(PatternID::must(0), 12)]);
        cmd.wait()?.expect_success();
    }

    let comparison = Comparison::default();
    let compare_result = comparison.compare(&backup_files, &restore_dir.join(&backup_files))?;
    dbg!(&compare_result);

    // no differences
    assert!(compare_result.is_empty());

    Ok(())
}
