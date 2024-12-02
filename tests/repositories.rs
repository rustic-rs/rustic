use anyhow::Result;
use assert_cmd::Command;
use flate2::read::GzDecoder;
use rstest::{fixture, rstest};
use rustic_testing::TestResult;
use std::{fs::File, path::Path};
use tar::Archive;
use tempfile::{tempdir, TempDir};

#[derive(Debug)]
pub struct TestSource(TempDir);

impl TestSource {
    pub fn new(tmp: TempDir) -> Self {
        Self(tmp)
    }

    pub fn into_path(self) -> TempDir {
        self.0
    }
}

fn open_and_unpack(open_path: &'static str, unpack_dir: &TempDir) -> Result<()> {
    let path = Path::new(open_path).canonicalize()?;
    let tar_gz = File::open(path)?;
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);
    archive.set_preserve_permissions(true);
    archive.set_preserve_mtime(true);
    archive.unpack(unpack_dir)?;
    Ok(())
}

#[fixture]
fn rustic_repo() -> Result<TestSource> {
    let dir = tempdir()?;
    let path = "tests/repository-fixtures/rustic-repo.tar.gz";
    open_and_unpack(path, &dir)?;
    Ok(TestSource::new(dir))
}

#[fixture]
fn restic_repo() -> Result<TestSource> {
    let dir = tempdir()?;
    let path = "tests/repository-fixtures/restic-repo.tar.gz";
    open_and_unpack(path, &dir)?;
    Ok(TestSource::new(dir))
}

#[fixture]
fn rustic_copy_repo() -> Result<TestSource> {
    let dir = tempdir()?;
    let path = "tests/repository-fixtures/rustic-copy-repo.tar.gz";
    open_and_unpack(path, &dir)?;

    Ok(TestSource::new(dir))
}

#[fixture]
pub fn src_snapshot() -> Result<TestSource> {
    let dir = tempdir()?;
    let path = "tests/repository-fixtures/src-snapshot.tar.gz";
    open_and_unpack(path, &dir)?;
    Ok(TestSource::new(dir))
}

pub fn rustic_runner(temp_dir: &Path, password: &'static str) -> TestResult<Command> {
    let repo_dir = temp_dir.join("repo");
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
    let rustic_repo_path = rustic_repo_path.path();

    {
        let mut runner = rustic_runner(rustic_repo_path, repo_password)?;
        runner.args(["check", "--read-data"]).assert().success();
    }

    {
        let mut runner = rustic_runner(rustic_repo_path, repo_password)?;
        runner
            .arg("snapshots")
            .assert()
            .success()
            .stdout(predicates::str::contains("2 snapshot(s)"));
    }

    {
        let mut runner = rustic_runner(rustic_repo_path, repo_password)?;
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
    let restic_repo_path = restic_repo_path.path();

    {
        let mut runner = rustic_runner(restic_repo_path, repo_password)?;
        runner.args(["check", "--read-data"]).assert().success();
    }

    {
        let mut runner = rustic_runner(restic_repo_path, repo_password)?;
        runner
            .arg("snapshots")
            .assert()
            .success()
            .stdout(predicates::str::contains("2 snapshot(s)"));
    }

    {
        let mut runner = rustic_runner(restic_repo_path, repo_password)?;
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

#[rstest]
#[ignore = "requires live fixture, run manually in CI"]
fn test_restic_latest_repo_with_rustic_passes() -> TestResult<()> {
    let path = "tests/repository-fixtures/";
    let repo_password = "restic";
    let restic_repo_path = Path::new(path).canonicalize()?;
    let restic_repo_path = restic_repo_path.as_path();

    {
        let mut runner = rustic_runner(restic_repo_path, repo_password)?;
        runner.args(["check", "--read-data"]).assert().success();
    }

    {
        let mut runner = rustic_runner(restic_repo_path, repo_password)?;
        runner
            .arg("snapshots")
            .assert()
            .success()
            .stdout(predicates::str::contains("2 snapshot(s)"));
    }

    Ok(())
}
