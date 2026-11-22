use assert_cmd::Command;
use rustic_testing::TestResult;
use tempfile::{TempDir, tempdir};

fn rustic_runner(temp_dir: &TempDir) -> TestResult<Command> {
    let mut runner = Command::new(env!("CARGO_BIN_EXE_rustic"));
    runner.arg("-r").arg(temp_dir.path().join("repo")).args([
        "--password",
        "test",
        "--no-progress",
    ]);
    Ok(runner)
}

#[test]
fn backup_json_writes_one_snapshot_per_line() -> TestResult<()> {
    let temp_dir = tempdir()?;
    let first_source = temp_dir.path().join("first-source");
    let second_source = temp_dir.path().join("second-source");
    std::fs::create_dir(&first_source)?;
    std::fs::create_dir(&second_source)?;
    std::fs::write(first_source.join("first.txt"), "first")?;
    std::fs::write(second_source.join("second.txt"), "second")?;

    rustic_runner(&temp_dir)?.arg("init").assert().success();

    let profile = temp_dir.path().join("two-sources.toml");
    let first_source = serde_json::to_string(&first_source.to_string_lossy())?;
    let second_source = serde_json::to_string(&second_source.to_string_lossy())?;
    std::fs::write(
        &profile,
        format!(
            "[[backup.snapshots]]\nsources = [{first_source}]\n\n[[backup.snapshots]]\nsources = [{second_source}]\n"
        ),
    )?;

    let output = rustic_runner(&temp_dir)?
        .arg("-P")
        .arg(profile)
        .args(["backup", "--json"])
        .output()?;
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout)?;
    assert!(
        stdout.ends_with('\n'),
        "JSON output is not line-terminated: {stdout}"
    );
    let lines: Vec<_> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "unexpected JSON output: {stdout}");
    let snapshots: Vec<serde_json::Value> = lines
        .iter()
        .map(|line| serde_json::from_str(line))
        .collect::<Result<_, _>>()?;
    assert_eq!(snapshots.len(), 2);

    Ok(())
}
