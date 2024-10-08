use anyhow::Result;
use assert_cmd::Command;
use flate2::read::GzDecoder;
use rstest::{fixture, rstest};
use rustic_testing::TestResult;
use std::{fs::File, path::Path};
use tar::Archive;
use tempfile::{tempdir, TempDir};

#[derive(Debug)]
struct TestSource(TempDir);

impl TestSource {
    pub fn new(tmp: TempDir) -> Self {
        Self(tmp)
    }

    pub fn into_path(self) -> TempDir {
        self.0
    }
}

#[fixture]
fn rustic_repo() -> Result<TestSource> {
    let dir = tempdir()?;
    let path = Path::new("tests/repository-fixtures/rustic-repo.tar.gz").canonicalize()?;
    let tar_gz = File::open(path)?;
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);
    archive.set_preserve_permissions(true);
    archive.set_preserve_mtime(true);
    archive.unpack(&dir)?;
    Ok(TestSource::new(dir))
}

#[fixture]
fn restic_repo() -> Result<TestSource> {
    let dir = tempdir()?;
    let path = Path::new("tests/repository-fixtures/restic-repo.tar.gz").canonicalize()?;
    let tar_gz = File::open(path)?;
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);
    archive.set_preserve_permissions(true);
    archive.set_preserve_mtime(true);
    archive.unpack(&dir)?;
    Ok(TestSource::new(dir))
}

#[fixture]
fn rustic_copy_repo() -> Result<TestSource> {
    let dir = tempdir()?;
    let path = Path::new("tests/repository-fixtures/rustic-copy-repo.tar.gz").canonicalize()?;
    let tar_gz = File::open(path)?;
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);
    archive.set_preserve_permissions(true);
    archive.set_preserve_mtime(true);
    archive.unpack(&dir)?;
    Ok(TestSource::new(dir))
}

#[fixture]
fn src_snapshot() -> Result<TestSource> {
    let dir = tempdir()?;
    let path = Path::new("tests/repository-fixtures/src-snapshot.tar.gz").canonicalize()?;
    let tar_gz = File::open(path)?;
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);
    archive.set_preserve_permissions(true);
    archive.set_preserve_mtime(true);
    archive.unpack(&dir)?;
    Ok(TestSource::new(dir))
}

pub fn rustic_runner(temp_dir: &TempDir, password: &'static str) -> TestResult<Command> {
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

#[rstest]
fn test_rustic_repo_passes(rustic_repo: Result<TestSource>) -> TestResult<()> {
    let rustic_repo = rustic_repo?;
    let repo_password = "rustic";
    let rustic_repo_path = rustic_repo.into_path();

    {
        let mut runner = rustic_runner(&rustic_repo_path, repo_password)?;
        runner.args(["check", "--read-data"]).assert().success();
    }

    {
        let mut runner = rustic_runner(&rustic_repo_path, repo_password)?;
        runner
            .arg("snapshots")
            .assert()
            .success()
            .stdout(predicates::str::contains("2 snapshot(s)"));
    }

    {
        let mut runner = rustic_runner(&rustic_repo_path, repo_password)?;
        runner
            .arg("diff")
            .arg("31d477a2")
            .arg("86371783")
            .assert()
            .success()
            .stdout(predicates::str::contains("1 removed"));
    }

    Ok(())
}

#[rstest]
fn test_restic_repo_with_rustic_passes(restic_repo: Result<TestSource>) -> TestResult<()> {
    let restic_repo = restic_repo?;
    let repo_password = "restic";
    let restic_repo_path = restic_repo.into_path();

    {
        let mut runner = rustic_runner(&restic_repo_path, repo_password)?;
        runner.args(["check", "--read-data"]).assert().success();
    }

    {
        let mut runner = rustic_runner(&restic_repo_path, repo_password)?;
        runner
            .arg("snapshots")
            .assert()
            .success()
            .stdout(predicates::str::contains("2 snapshot(s)"));
    }

    {
        let mut runner = rustic_runner(&restic_repo_path, repo_password)?;
        runner
            .arg("diff")
            .arg("9305509c")
            .arg("af05ecb6")
            .assert()
            .success()
            .stdout(predicates::str::contains("1 removed"));
    }

    Ok(())
}
