//! Regression coverage for the human backup summary in log files.

use std::fs;

use assert_cmd::Command;
use rustic_testing::TestResult;
use tempfile::{tempdir, TempDir};

fn rustic_runner(temp_dir: &TempDir) -> TestResult<Command> {
    let repo_dir = temp_dir.path().join("repo");
    let mut runner = Command::new(env!("CARGO_BIN_EXE_rustic"));

    runner
        .arg("--repo")
        .arg(repo_dir)
        .arg("--password")
        .arg("test")
        .arg("--no-progress");

    Ok(runner)
}

fn setup() -> TestResult<TempDir> {
    let temp_dir = tempdir()?;
    rustic_runner(&temp_dir)?.arg("init").assert().success();
    Ok(temp_dir)
}

#[test]
fn backup_log_includes_the_human_summary() -> TestResult<()> {
    let temp_dir = setup()?;
    let source = temp_dir.path().join("source");
    let nested = source.join("nested");
    let log_file = temp_dir.path().join("backup.log");
    fs::create_dir_all(&nested)?;
    fs::write(nested.join("payload.txt"), "payload")?;

    rustic_runner(&temp_dir)?
        .arg("--log-file")
        .arg(&log_file)
        .arg("backup")
        .arg(&source)
        .assert()
        .success();

    let log = fs::read_to_string(&log_file)?;
    for summary in [
        "[INFO] - Files:",
        "[INFO] - Dirs:",
        "[INFO] - Added to the repo:",
        "[INFO] - processed ",
    ] {
        assert!(
            log.contains(summary),
            "backup log is missing `{summary}`: {log}"
        );
    }
    assert!(
        log.lines().any(
            |line| line.contains("[INFO] - snapshot ") && line.contains(" successfully saved.")
        ),
        "backup log is missing the saved snapshot summary: {log}"
    );

    Ok(())
}
