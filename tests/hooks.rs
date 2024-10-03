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
fn test_global_empty_hooks_passes() -> TestResult<()> {
    let hooks_config = hook_fixture_dir().join("empty_hooks.toml");

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
    let hooks_config = hook_fixture_dir().join("global_hooks");

    let temp_dir = setup()?;
    let live_log = std::env::current_dir()?.join("global_hooks.log");

    {
        rustic_runner(&temp_dir)?
            .args(["-P", hooks_config.to_str().unwrap()])
            .arg("repoinfo")
            .assert()
            .success();
    }

    // compare the content of the global hook log with our fixture
    let global_log_fixture_content =
        std::fs::read_to_string(hook_fixture_dir().join("global_hooks_success.log"))?;
    let global_log_live = std::fs::read_to_string(&live_log)?;
    remove_file(live_log)?;
    assert_eq!(global_log_fixture_content, global_log_live);

    Ok(())
}

#[test]
fn test_repository_hooks_passes() -> TestResult<()> {
    let hooks_config = hook_fixture_dir().join("repository_hooks");

    let temp_dir = setup()?;
    let live_log = std::env::current_dir()?.join("repository_hooks.log");

    {
        rustic_runner(&temp_dir)?
            .args(["-P", hooks_config.to_str().unwrap()])
            .arg("check")
            .assert()
            .success();
    }

    // compare the content of the repo hook log with our fixture
    let repo_log_fixture_content =
        std::fs::read_to_string(hook_fixture_dir().join("repository_hooks_success.log"))?;
    let repo_log_live = std::fs::read_to_string(&live_log)?;
    remove_file(live_log)?;
    assert_eq!(repo_log_fixture_content, repo_log_live);

    Ok(())
}

#[test]
fn test_backup_hooks_passes() -> TestResult<()> {
    let hooks_config = hook_fixture_dir().join("backup_hooks");
    let backup = "src/";
    let temp_dir = setup()?;
    let live_log = std::env::current_dir()?.join("backup_hooks.log");

    {
        rustic_runner(&temp_dir)?
            .args(["-P", hooks_config.to_str().unwrap()])
            .arg("backup")
            .arg(backup)
            .assert()
            .success();
    }

    // compare the content of the backup hook log with our fixture
    let backup_log_fixture_content =
        std::fs::read_to_string(hook_fixture_dir().join("backup_hooks_success.log"))?;
    let backup_log_live = std::fs::read_to_string(&live_log)?;
    remove_file(live_log)?;
    assert_eq!(backup_log_fixture_content, backup_log_live);

    Ok(())
}
