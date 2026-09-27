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
fn snapshots_long_renders_crlf_description_inside_table_cells() -> TestResult<()> {
    let temp_dir = tempdir()?;
    let source = temp_dir.path().join("source");
    let description = temp_dir.path().join("description.txt");
    std::fs::create_dir(&source)?;
    std::fs::write(source.join("file.txt"), "backup data")?;
    std::fs::write(&description, "first line\r\nsecond line\r\n")?;

    rustic_runner(&temp_dir)?.arg("init").assert().success();
    rustic_runner(&temp_dir)?
        .args(["backup", "--description-from"])
        .arg(&description)
        .arg(&source)
        .assert()
        .success();

    let output = rustic_runner(&temp_dir)?
        .args(["snapshots", "--long"])
        .output()?;
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout)?;
    // Strip platform line terminators first so this also runs on Windows, while
    // still detecting a carriage return emitted from the description itself.
    let stdout = stdout.replace("\r\n", "\n");
    assert!(
        !stdout.contains('\r'),
        "table output contains a carriage return: {stdout}"
    );
    let rows: Vec<_> = stdout
        .lines()
        .filter(|line| line.starts_with('|'))
        .collect();
    assert!(
        rows.iter()
            .any(|line| line.contains("Description") && line.contains("first line")),
        "first description line is not inside a table cell: {stdout}"
    );
    assert!(
        rows.iter().any(|line| line.contains("second line")),
        "second description line is not inside a table cell: {stdout}"
    );
    assert!(
        rows.iter().all(|line| line.ends_with('|')),
        "table row is missing its closing cell border: {stdout}"
    );

    Ok(())
}
