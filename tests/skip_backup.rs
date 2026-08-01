//! Integration coverage for conditionally omitting configured backup sources.

#![cfg(unix)]

use assert_cmd::Command;
use predicates::prelude::predicate;
use tempfile::{TempDir, tempdir};

use rustic_testing::TestResult;

fn rustic_runner(temp_dir: &TempDir) -> TestResult<Command> {
    let mut runner = Command::new(env!("CARGO_BIN_EXE_rustic"));
    runner
        .arg("-r")
        .arg(temp_dir.path().join("repo"))
        .arg("--password")
        .arg("test")
        .arg("--no-progress");
    Ok(runner)
}

fn setup() -> TestResult<TempDir> {
    let temp_dir = tempdir()?;
    rustic_runner(&temp_dir)?.args(["init"]).assert().success();
    Ok(temp_dir)
}

fn assert_snapshot_count(temp_dir: &TempDir, expected: usize) -> TestResult<()> {
    rustic_runner(temp_dir)?
        .args(["snapshots"])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "total: {expected} snapshot(s)"
        )));
    Ok(())
}

#[test]
fn skip_if_command_requires_an_explicit_safe_directive() -> TestResult<()> {
    let temp_dir = setup()?;
    let source = temp_dir.path().join("source");
    std::fs::create_dir(&source)?;
    std::fs::write(source.join("important.txt"), "important data")?;

    let profile = temp_dir.path().join("backup.toml");
    std::fs::write(
        &profile,
        r#"
            [backup]
            skip-if-command = "sh -c 'printf skip'"
        "#,
    )?;
    rustic_runner(&temp_dir)?
        .args(["-P", profile.to_str().unwrap(), "backup"])
        .arg(&source)
        .assert()
        .success()
        .stderr(predicate::str::contains("skipping backup of"));
    assert_snapshot_count(&temp_dir, 0)?;

    std::fs::write(
        &profile,
        r#"
            [backup]
            skip-if-command = "sh -c ':'"
        "#,
    )?;
    rustic_runner(&temp_dir)?
        .args(["-P", profile.to_str().unwrap(), "backup"])
        .arg(&source)
        .assert()
        .success();
    assert_snapshot_count(&temp_dir, 1)?;

    std::fs::write(
        &profile,
        r#"
            [backup]
            skip-if-command = "sh -c 'printf unexpected'"
        "#,
    )?;
    rustic_runner(&temp_dir)?
        .args(["-P", profile.to_str().unwrap(), "backup"])
        .arg(&source)
        .assert()
        .failure()
        .stderr(predicate::str::contains("must write exactly `skip`"));
    assert_snapshot_count(&temp_dir, 1)?;

    std::fs::write(
        &profile,
        r#"
            [backup]
            skip-if-command = "sh -c 'exit 12'"
        "#,
    )?;
    rustic_runner(&temp_dir)?
        .args(["-P", profile.to_str().unwrap(), "backup"])
        .arg(&source)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "skip-if-command` exited with status",
        ));
    assert_snapshot_count(&temp_dir, 1)?;

    Ok(())
}
