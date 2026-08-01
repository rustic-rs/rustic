//! Regression tests for `.gitignore` handling during backups.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
};

use assert_cmd::Command;
use rustic_testing::TestResult;
use tempfile::{tempdir, TempDir};

fn rustic_runner(temp_dir: &TempDir, repository_name: &str) -> TestResult<Command> {
    let repo_dir = temp_dir.path().join(repository_name);
    let mut runner = Command::new(env!("CARGO_BIN_EXE_rustic"));

    runner
        .arg("--repo")
        .arg(repo_dir)
        .arg("--password")
        .arg("test")
        .arg("--no-progress");

    Ok(runner)
}

fn init_repository(temp_dir: &TempDir, repository_name: &str) -> TestResult<()> {
    rustic_runner(temp_dir, repository_name)?
        .arg("init")
        .assert()
        .success();
    Ok(())
}

fn init_git_worktree(path: &Path) -> TestResult<()> {
    let output = ProcessCommand::new("git")
        .args(["init", "--quiet"])
        .current_dir(path)
        .output()?;
    assert!(
        output.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

fn backup_and_restore(
    temp_dir: &TempDir,
    repository_name: &str,
    source: &Path,
    no_require_git: bool,
) -> TestResult<PathBuf> {
    init_repository(temp_dir, repository_name)?;

    let mut backup = rustic_runner(temp_dir, repository_name)?;
    backup.arg("backup").arg("--git-ignore");
    if no_require_git {
        backup.arg("--no-require-git");
    }
    backup
        .args(["--as-path", "/"])
        .arg(source)
        .assert()
        .success();

    let restore_dir = temp_dir.path().join(format!("restore-{repository_name}"));
    rustic_runner(temp_dir, repository_name)?
        .args(["restore", "latest"])
        .arg(&restore_dir)
        .assert()
        .success();

    Ok(restore_dir)
}

#[test]
fn parent_gitignore_rules_apply_to_a_nested_backup_source_in_a_worktree() -> TestResult<()> {
    let temp_dir = tempdir()?;
    let worktree = temp_dir.path().join("worktree");
    let source = worktree.join("service");
    let nested = source.join("cache");
    let ignored_dir = source.join("ignored-directory");
    fs::create_dir_all(&nested)?;
    fs::create_dir_all(&ignored_dir)?;
    fs::write(worktree.join(".gitignore"), "*.log\nignored-directory/\n")?;
    fs::write(source.join("kept.txt"), "keep")?;
    fs::write(nested.join("build.log"), "ignore")?;
    fs::write(ignored_dir.join("ignored.txt"), "ignore")?;
    init_git_worktree(&worktree)?;

    let restore_dir = backup_and_restore(&temp_dir, "worktree-repo", &source, false)?;

    assert!(restore_dir.join("kept.txt").is_file());
    assert!(!restore_dir.join("cache/build.log").exists());
    assert!(!restore_dir.join("ignored-directory/ignored.txt").exists());

    Ok(())
}

#[test]
fn no_require_git_applies_gitignore_rules_outside_a_worktree() -> TestResult<()> {
    let temp_dir = tempdir()?;
    let source = temp_dir.path().join("non-git-source");
    let nested = source.join("nested");
    fs::create_dir_all(&nested)?;
    fs::write(source.join(".gitignore"), "*.log\n")?;
    fs::write(nested.join("kept.txt"), "keep")?;
    fs::write(nested.join("ignored.log"), "ignore")?;

    let requires_git_restore = backup_and_restore(&temp_dir, "requires-git-repo", &source, false)?;
    assert!(
        requires_git_restore.join("nested/ignored.log").is_file(),
        "a non-Git source should not apply .gitignore rules without --no-require-git"
    );

    let no_require_git_restore =
        backup_and_restore(&temp_dir, "no-require-git-repo", &source, true)?;
    assert!(no_require_git_restore.join("nested/kept.txt").is_file());
    assert!(!no_require_git_restore.join("nested/ignored.log").exists());

    Ok(())
}
