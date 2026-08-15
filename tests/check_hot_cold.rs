use std::path::Path;

use assert_cmd::Command;
use rustic_testing::TestResult;
use tempfile::tempdir;

fn rustic_runner(profile: &Path, cold_repo: &Path, hot_repo: &Path) -> TestResult<Command> {
    let mut runner = Command::new(env!("CARGO_BIN_EXE_rustic"));
    runner
        .arg("--use-profile")
        .arg(profile)
        .arg("--repository")
        .arg(cold_repo)
        .arg("--repo-hot")
        .arg(hot_repo)
        .args(["--password", "test", "--no-progress", "--no-cache"]);
    Ok(runner)
}

#[test]
fn check_read_data_uses_cold_data_packs_for_hot_cold_repositories() -> TestResult<()> {
    let temp_dir = tempdir()?;
    let cold_repo = temp_dir.path().join("cold-repository");
    let hot_repo = temp_dir.path().join("hot-repository");
    let source = temp_dir.path().join("source");
    let profile = temp_dir.path().join("test.toml");
    std::fs::write(&profile, "")?;
    std::fs::create_dir(&source)?;
    std::fs::write(source.join("payload.bin"), vec![42_u8; 256 * 1024])?;

    rustic_runner(&profile, &cold_repo, &hot_repo)?
        .arg("init")
        .assert()
        .success();

    rustic_runner(&profile, &cold_repo, &hot_repo)?
        .arg("backup")
        .arg(&source)
        .assert()
        .success();

    // Data packs are intentionally stored only in the cold repository. A
    // successful read-data check proves it reads them there rather than trying
    // the hot metadata repository.
    rustic_runner(&profile, &cold_repo, &hot_repo)?
        .args(["check", "--read-data"])
        .assert()
        .success();

    Ok(())
}
