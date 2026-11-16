//! Regression tests for `--glob-file` input handling.

use assert_cmd::Command;
use predicates::prelude::{PredicateBooleanExt, predicate};
use rustic_testing::TestResult;
use tempfile::{TempDir, tempdir};

fn rustic_runner(temp_dir: &TempDir) -> TestResult<Command> {
    let mut runner = Command::new(env!("CARGO_BIN_EXE_rustic"));
    runner
        .arg("--repo")
        .arg(temp_dir.path().join("repo"))
        .args(["--password", "test", "--no-progress"]);
    Ok(runner)
}

#[test]
fn glob_file_pattern_is_reported_as_an_input_error() -> TestResult<()> {
    let temp_dir = tempdir()?;
    let source = temp_dir.path().join("source");
    std::fs::create_dir(&source)?;
    rustic_runner(&temp_dir)?.arg("init").assert().success();

    let profile = temp_dir.path().join("backup.toml");
    for bad_glob_file in [
        format!("!{}/SteamLibrary", source.display()),
        source.display().to_string(),
    ] {
        let source_value = serde_json::to_string(&source.to_string_lossy())?;
        let glob_file_value = serde_json::to_string(&bad_glob_file)?;
        std::fs::write(
            &profile,
            format!(
                "[[backup.snapshots]]\nsources = [{source_value}]\nglob-files = [{glob_file_value}]\n"
            ),
        )?;

        rustic_runner(&temp_dir)?
            .arg("--use-profile")
            .arg(&profile)
            .arg("backup")
            .assert()
            .failure()
            .stderr(predicate::str::contains("failed to read glob file"))
            .stderr(predicate::str::contains(
                "expected a file containing glob patterns",
            ))
            .stderr(predicate::str::contains("We believe this is a bug").not());
    }

    Ok(())
}

#[test]
fn glob_file_created_by_before_hook_is_loaded() -> TestResult<()> {
    let temp_dir = tempdir()?;
    let source = temp_dir.path().join("source");
    let glob_file = temp_dir.path().join("generated.glob");
    std::fs::create_dir(&source)?;
    std::fs::write(source.join("file.txt"), "backup data")?;
    rustic_runner(&temp_dir)?.arg("init").assert().success();

    let profile = temp_dir.path().join("backup.toml");
    let source = serde_json::to_string(&source.to_string_lossy())?;
    let glob_file_json = serde_json::to_string(&glob_file.to_string_lossy())?;
    let create_glob_file =
        serde_json::to_string(&format!("sh -c 'touch \"{}\"'", glob_file.display()))?;
    std::fs::write(
        &profile,
        format!(
            "[[backup.snapshots]]\nsources = [{source}]\nglob-files = [{glob_file_json}]\nhooks = {{ run-before = [{create_glob_file}] }}\n"
        ),
    )?;

    assert!(!glob_file.exists());
    rustic_runner(&temp_dir)?
        .arg("--use-profile")
        .arg(&profile)
        .arg("backup")
        .assert()
        .success();
    assert!(glob_file.exists());

    Ok(())
}
