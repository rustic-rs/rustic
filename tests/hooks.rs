//! Hooks test: runs the application as a subprocess and asserts its
//! interaction with different files due to hooks

// #![forbid(unsafe_code)]
// #![warn(
//     missing_docs,
//     rust_2018_idioms,
//     trivial_casts,
//     unused_lifetimes,
//     unused_qualifications
// )]

use std::path::PathBuf;

use abscissa_core::fs::remove_file;
use assert_cmd::Command;
use predicates::prelude::predicate;
use tempfile::{tempdir, TempDir};

use rustic_testing::TestResult;

fn hook_fixture_dir() -> PathBuf {
    ["tests", "hooks-fixtures"].iter().collect()
}

fn generated_dir() -> PathBuf {
    ["tests", "generated"].iter().collect()
}

fn toml_fixture_dir() -> PathBuf {
    hook_fixture_dir().join("toml")
}

fn log_fixture_dir() -> PathBuf {
    hook_fixture_dir().join("log")
}

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

/// Compare the content of the repo hook log with our fixture
fn compare_logs(log_fixture_path: PathBuf, log_live_path: PathBuf) -> TestResult<()> {
    let log_fixture_content = std::fs::read_to_string(log_fixture_path)?;
    let log_live = std::fs::read_to_string(&log_live_path)?;
    remove_file(log_live_path)?;
    assert_eq!(log_fixture_content, log_live);
    Ok(())
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum RunnerStatus {
    Success,
    Failure,
}

fn run_hook_comparison(
    temp_dir: TempDir,
    hooks_config: PathBuf,
    args: &[&str],
    log_fixture_path: PathBuf,
    log_live_path: PathBuf,
    status: RunnerStatus,
) -> TestResult<()> {
    {
        let runner = rustic_runner(&temp_dir)?
            .args(["-P", hooks_config.to_str().unwrap()])
            .args(args)
            .assert();

        match status {
            RunnerStatus::Success => runner.success(),
            RunnerStatus::Failure => runner.failure(),
        };
    }

    compare_logs(log_fixture_path, log_live_path)?;

    Ok(())
}

#[test]
fn test_empty_hooks_passes() -> TestResult<()> {
    let hooks_config = toml_fixture_dir().join("empty_hooks_success");

    let temp_dir = setup()?;

    {
        rustic_runner(&temp_dir)?
            .args(["-P", hooks_config.to_str().unwrap()])
            .arg("repoinfo")
            .assert()
            .success()
            .stdout(predicate::str::contains("Total Size"));
    }

    Ok(())
}

#[test]
fn test_global_hooks_passes() -> TestResult<()> {
    let hooks_config_path = toml_fixture_dir().join("global_hooks_success");

    let temp_dir = setup()?;

    let args = &["repoinfo"];

    let file_name = "global_hooks_success.log";
    let log_live_path = generated_dir().join(file_name);
    let log_fixture_path = log_fixture_dir().join(file_name);

    run_hook_comparison(
        temp_dir,
        hooks_config_path,
        args,
        log_fixture_path,
        log_live_path,
        RunnerStatus::Success,
    )?;

    Ok(())
}

#[test]
fn test_repository_hooks_passes() -> TestResult<()> {
    let hooks_config_path = toml_fixture_dir().join("repository_hooks_success");

    let temp_dir = setup()?;

    let args = &["check"];

    let file_name = "repository_hooks_success.log";
    let log_live_path = generated_dir().join(file_name);
    let log_fixture_path = log_fixture_dir().join(file_name);

    run_hook_comparison(
        temp_dir,
        hooks_config_path,
        args,
        log_fixture_path,
        log_live_path,
        RunnerStatus::Success,
    )?;

    Ok(())
}

#[test]
fn test_backup_hooks_passes() -> TestResult<()> {
    let hooks_config_path = toml_fixture_dir().join("backup_hooks_success");

    let temp_dir = setup()?;
    let args = &["backup", "src/"];

    let file_name = "backup_hooks_success.log";
    let log_live_path = generated_dir().join(file_name);
    let log_fixture_path = log_fixture_dir().join(file_name);

    run_hook_comparison(
        temp_dir,
        hooks_config_path,
        args,
        log_fixture_path,
        log_live_path,
        RunnerStatus::Success,
    )?;

    Ok(())
}

#[test]
fn test_full_hooks_passes() -> TestResult<()> {
    let hooks_config_path = toml_fixture_dir().join("full_hooks_success");
    let temp_dir = setup()?;
    let args = &["backup", "src/"];

    let file_name = "full_hooks_success.log";
    let log_live_path = generated_dir().join(file_name);
    let log_fixture_path = log_fixture_dir().join(file_name);

    run_hook_comparison(
        temp_dir,
        hooks_config_path,
        args,
        log_fixture_path,
        log_live_path,
        RunnerStatus::Success,
    )?;

    Ok(())
}

#[test]
fn test_backup_hooks_with_failure_passes() -> TestResult<()> {
    let hooks_config_path = toml_fixture_dir().join("backup_hooks_failure");
    let temp_dir = setup()?;
    let args = &["backup", "src/"];

    let file_name = "backup_hooks_failure.log";
    let log_live_path = generated_dir().join(file_name);
    let log_fixture_path = log_fixture_dir().join(file_name);

    run_hook_comparison(
        temp_dir,
        hooks_config_path,
        args,
        log_fixture_path,
        log_live_path,
        RunnerStatus::Failure,
    )?;

    Ok(())
}
