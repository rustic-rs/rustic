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

use assert_cmd::Command;
use predicates::prelude::{predicate, PredicateBooleanExt};
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
    let hook_dir = hook_fixture_dir();
    let hooks_config = hook_dir.join("empty_hooks.toml");

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
    let hook_dir = hook_fixture_dir();
    let hooks_config = hook_dir.join("global_hooks.toml");

    let temp_dir = setup()?;

    {
        rustic_runner(&temp_dir)?
            .args(["-P", hooks_config.to_str().unwrap()])
            .arg("repoinfo")
            .assert()
            .success()
            .stdout(predicate::str::contains("Total Size"))
            .stdout(predicate::str::contains("Running global hooks before"))
            .stdout(predicate::str::contains("Running global hooks after"))
            .stdout(predicate::str::contains("Running global hooks failed").not())
            .stdout(predicate::str::contains("Running global hooks finally"));
    }

    Ok(())
}

#[test]
fn test_repository_hooks_passes() -> TestResult<()> {
    let hook_dir = hook_fixture_dir();
    let hooks_config = hook_dir.join("repository_hooks.toml");

    let temp_dir = setup()?;

    {
        rustic_runner(&temp_dir)?
            .args(["-P", hooks_config.to_str().unwrap()])
            .arg("check")
            .assert()
            .success()
            .stdout(predicate::str::contains("Running repository hooks before"))
            .stdout(predicate::str::contains("Running repository hooks after"))
            .stdout(predicate::str::contains("Running repository hooks failed").not())
            .stdout(predicate::str::contains("Running repository hooks finally"));
    }

    Ok(())
}

#[test]
fn test_backup_hooks_passes() -> TestResult<()> {
    let hook_dir = hook_fixture_dir();
    let hooks_config = hook_dir.join("backup_hooks.toml");
    let backup = "src/";
    let temp_dir = setup()?;

    {
        rustic_runner(&temp_dir)?
            .args(["-P", hooks_config.to_str().unwrap()])
            .arg("backup")
            .arg(backup)
            .assert()
            .success()
            .stdout(predicate::str::contains("Running backup hooks before"))
            .stdout(predicate::str::contains("Running backup hooks after"))
            .stdout(predicate::str::contains("Running backup hooks failed").not())
            .stdout(predicate::str::contains("Running backup hooks finally"));
    }

    Ok(())
}
