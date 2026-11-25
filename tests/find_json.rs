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
fn find_json_reports_full_snapshot_id_and_matching_node() -> TestResult<()> {
    let temp_dir = tempdir()?;
    let source = temp_dir.path().join("source");
    std::fs::create_dir(&source)?;
    std::fs::write(source.join("found.txt"), "contents")?;

    rustic_runner(&temp_dir)?.arg("init").assert().success();
    let backup_output = rustic_runner(&temp_dir)?
        .args(["backup", "--json"])
        .arg(&source)
        .output()?;
    assert!(backup_output.status.success());
    let snapshot: serde_json::Value = serde_json::from_slice(&backup_output.stdout)?;
    let snapshot_id = snapshot["id"].as_str().unwrap();

    let output = rustic_runner(&temp_dir)?
        .args(["find", "--glob", "*.txt", "--json"])
        .output()?;
    assert!(output.status.success());

    let results: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let results = results.as_array().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["snapshot"], snapshot_id);
    assert_eq!(results[0]["matches"].as_array().unwrap().len(), 1);
    assert_eq!(results[0]["matches"][0]["name"], "found.txt");
    assert_eq!(results[0]["matches"][0]["type"], "file");

    Ok(())
}
