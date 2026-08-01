//! Integration tests for backup include/exclude glob rules.

use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

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

fn backup_and_restore<I, S>(
    temp_dir: &TempDir,
    source: &Path,
    exclude_args: I,
) -> TestResult<PathBuf>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    rustic_runner(temp_dir)?
        .arg("backup")
        .args(exclude_args)
        .args(["--as-path", "/"])
        .arg(source)
        .assert()
        .success();

    let restore_dir = temp_dir.path().join("restore");
    rustic_runner(temp_dir)?
        .args(["restore", "latest"])
        .arg(&restore_dir)
        .assert()
        .success();

    Ok(restore_dir)
}

#[test]
fn glob_rules_can_include_a_group_and_exclude_one_member() -> TestResult<()> {
    let temp_dir = setup()?;
    let source = temp_dir.path().join("source");
    fs::create_dir(&source)?;
    fs::write(source.join("included.txt"), "include")?;
    fs::write(source.join("excluded.txt"), "exclude")?;
    fs::write(source.join("unselected.log"), "unselected")?;

    let restore_dir = backup_and_restore(
        &temp_dir,
        &source,
        ["--glob", "**/*.txt", "--glob", "!**/excluded.txt"],
    )?;

    assert!(restore_dir.join("included.txt").is_file());
    assert!(!restore_dir.join("excluded.txt").exists());
    assert!(!restore_dir.join("unselected.log").exists());

    Ok(())
}

#[test]
fn iglob_rules_ignore_filename_case() -> TestResult<()> {
    let temp_dir = setup()?;
    let source = temp_dir.path().join("source");
    fs::create_dir(&source)?;
    fs::write(source.join("Case-Only.TMP"), "exclude")?;
    fs::write(source.join("kept.txt"), "keep")?;

    let restore_dir = backup_and_restore(&temp_dir, &source, ["--iglob", "!**/case-only.tmp"])?;

    assert!(!restore_dir.join("Case-Only.TMP").exists());
    assert!(restore_dir.join("kept.txt").is_file());

    Ok(())
}

#[test]
fn glob_file_applies_every_pattern_line() -> TestResult<()> {
    let temp_dir = setup()?;
    let source = temp_dir.path().join("source");
    let nested = source.join("nested");
    let glob_file = temp_dir.path().join("backup.globs");
    fs::create_dir_all(&nested)?;
    fs::write(source.join("kept.txt"), "keep")?;
    fs::write(source.join("discard.cache"), "exclude")?;
    fs::write(nested.join("public.txt"), "keep")?;
    fs::write(nested.join("private.txt"), "exclude")?;
    fs::write(&glob_file, "!**/*.cache\n!**/nested/private.txt\n")?;

    let restore_dir = backup_and_restore(
        &temp_dir,
        &source,
        [OsStr::new("--glob-file"), glob_file.as_os_str()],
    )?;

    assert!(restore_dir.join("kept.txt").is_file());
    assert!(!restore_dir.join("discard.cache").exists());
    assert!(restore_dir.join("nested/public.txt").is_file());
    assert!(!restore_dir.join("nested/private.txt").exists());

    Ok(())
}
