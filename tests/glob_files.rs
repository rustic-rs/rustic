//! Regression tests for glob-file input handling.

use assert_cmd::Command;
use predicates::prelude::{PredicateBooleanExt, predicate};
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
fn directory_passed_as_glob_file_is_a_normal_input_error() -> TestResult<()> {
    let temp_dir = setup()?;
    let source = temp_dir.path().join("source");
    let glob_directory = temp_dir.path().join("not-a-glob-file");
    std::fs::create_dir(&source)?;
    std::fs::create_dir(&glob_directory)?;

    rustic_runner(&temp_dir)?
        .args(["backup", "--glob-file"])
        .arg(&glob_directory)
        .arg(&source)
        .assert()
        .failure()
        .stderr(
            predicate::str::contains(format!(
                "failed to read glob file `{}`",
                glob_directory.display()
            ))
            .and(predicate::str::contains(
                "expected a file containing glob patterns",
            ))
            .and(predicate::str::contains("We believe this is a bug").not()),
        );

    Ok(())
}

#[test]
fn glob_file_patterns_are_applied() -> TestResult<()> {
    let temp_dir = setup()?;
    let source = temp_dir.path().join("source");
    let glob_file = temp_dir.path().join("excludes.glob");
    std::fs::create_dir(&source)?;
    std::fs::write(source.join("included.txt"), "included")?;
    std::fs::write(source.join("excluded.txt"), "excluded")?;
    std::fs::write(&glob_file, "!**/excluded.txt\n")?;

    let output = rustic_runner(&temp_dir)?
        .args(["backup", "--ls", "--glob-file"])
        .arg(&glob_file)
        .arg(&source)
        .output()?;

    assert!(
        output.status.success(),
        "backup --ls failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("included.txt"), "unexpected output: {stdout}");
    assert!(
        !stdout.contains("excluded.txt"),
        "glob-file exclusion was ignored: {stdout}"
    );

    Ok(())
}
