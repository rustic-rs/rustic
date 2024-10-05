//! Hooks test: runs the application as a subprocess and asserts its
//! interaction with different files due to hooks

// #![forbid(unsafe_code)]
// #![warn(
//     missing_docs,
//     rust_2018_idioms,
//     trivial_casts,
//     unused_lifetimes,
//     unused_qualifications
// )]

use std::path::PathBuf;

use abscissa_core::fs::remove_file;
use assert_cmd::Command;
use predicates::prelude::predicate;
use rstest::{fixture, rstest};
use tempfile::{tempdir, TempDir};

use rustic_testing::TestResult;
#[fixture]
fn hook_fixture_dir() -> PathBuf {
    ["tests", "hooks-fixtures"].iter().collect()
}

#[fixture]
fn generated_dir() -> PathBuf {
    ["tests", "generated"].iter().collect()
}

#[fixture]
fn toml_fixture_dir() -> PathBuf {
    hook_fixture_dir()
}

#[fixture]
fn log_fixture_dir() -> PathBuf {
    hook_fixture_dir().join("log")
}

pub fn rustic_runner(temp_dir: &TempDir) -> TestResult<Command> {
    let password = "test";
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

#[allow(dead_code)]
enum BackupAction {
    WithBackup,
    WithoutBackup,
}

// Load a template from a file and replace a placeholder with a value
// and write the result to a new file
fn load_template_replace_and_write(
    template_path: &PathBuf,
    placeholder: &str,
    value: &str,
    output_path: &PathBuf,
) -> TestResult<()> {
    let template = std::fs::read_to_string(template_path)?;
    let replaced = template.replace(placeholder, value);
    std::fs::write(output_path, replaced)?;

    Ok(())
}

fn setup(with_backup: BackupAction) -> TestResult<TempDir> {
    let temp_dir = tempdir()?;
    rustic_runner(&temp_dir)?
        .args(["init"])
        .assert()
        .success()
        .stderr(predicate::str::contains("successfully created."))
        .stderr(predicate::str::contains("successfully added."));

    match with_backup {
        BackupAction::WithBackup => {
            rustic_runner(&temp_dir)?
                // We need this so output on stderr is not being taken as an error
                .arg("--log-level=error")
                .args(["backup", "src/"])
                .assert()
                .success();
        }
        BackupAction::WithoutBackup => {}
    }

    Ok(temp_dir)
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum RunnerStatus {
    Success,
    Failure,
}

fn run_hook_comparison(
    temp_dir: TempDir,
    hooks_config: PathBuf,
    args: &[&str],
    snapshot_name: &str,
    log_live_path: PathBuf,
    status: RunnerStatus,
) -> TestResult<()> {
    {
        let runner = rustic_runner(&temp_dir)?
            // We need this so output on stderr is not being taken as an error
            .arg("--log-level=error")
            .args(["-P", hooks_config.to_str().unwrap()])
            .args(args)
            .assert();

        match status {
            RunnerStatus::Success => runner.success(),
            RunnerStatus::Failure => runner.failure(),
        };
    }

    let log_live = std::fs::read_to_string(&log_live_path)?;
    remove_file(log_live_path)?;
    insta::assert_ron_snapshot!(snapshot_name, log_live);

    Ok(())
}

#[rstest]
fn test_empty_hooks_do_nothing_passes(toml_fixture_dir: PathBuf) -> TestResult<()> {
    let hooks_config = toml_fixture_dir.join("empty_hooks_success");

    let temp_dir = setup(BackupAction::WithoutBackup)?;

    {
        rustic_runner(&temp_dir)?
            .args(["-P", hooks_config.to_str().unwrap()])
            .arg("repoinfo")
            .assert()
            .success()
            .stdout(predicate::str::contains("Total Size"));
    }

    Ok(())
}

macro_rules! generate_test_hook_function {
    ($name:ident, $fixture:expr, $args:expr, $status:expr) => {
        #[rstest]
        fn $name(toml_fixture_dir: PathBuf, generated_dir: PathBuf) -> TestResult<()> {
            let hooks_config_path = toml_fixture_dir.join($fixture);
            let args = $args;

            let file_name = format!("{}.log", $fixture);
            let log_live_path = generated_dir.join(&file_name);

            run_hook_comparison(
                setup(BackupAction::WithoutBackup)?,
                hooks_config_path,
                args,
                $fixture,
                log_live_path,
                $status,
            )?;

            Ok(())
        }
    };
}

// Scenario: Global hooks pass in order
generate_test_hook_function!(
    test_global_hooks_order_passes,
    "global_hooks_success",
    &["repoinfo"],
    RunnerStatus::Success
);

// Scenario: Repository hooks pass in order
generate_test_hook_function!(
    test_repository_hooks_order_passes,
    "repository_hooks_success",
    &["check"],
    RunnerStatus::Success
);

// Scenario: Backup hooks pass in order
generate_test_hook_function!(
    test_backup_hooks_order_passes,
    "backup_hooks_success",
    &["backup", "src/"],
    RunnerStatus::Success
);

// Scenario: Full hooks pass in order
generate_test_hook_function!(
    test_full_hooks_order_passes,
    "full_hooks_success",
    &["backup", "src/"],
    RunnerStatus::Success
);

// Scenario: Check do not run backup hooks
generate_test_hook_function!(
    test_check_do_not_run_backup_hooks_passes,
    "check_not_backup_hooks_success",
    &["check"],
    RunnerStatus::Success
);

// Scenario: Failure in before backup hook does not run backup
generate_test_hook_function!(
    test_backup_hooks_with_failure_passes,
    "backup_hooks_failure",
    &["backup", "src/"],
    RunnerStatus::Failure
);

// Scenario: Failure in after backup hook does run repo and global
// hooks failed and finally
generate_test_hook_function!(
    test_full_hooks_with_failure_after_backup_passes,
    "full_hooks_after_backup_failure",
    &["backup", "src/"],
    RunnerStatus::Failure
);

// Scenario: Failure in before repo hook does run repo and global
// hooks failed and finally
generate_test_hook_function!(
    test_full_hooks_with_failure_before_repo_passes,
    "full_hooks_before_repo_failure",
    &["backup", "src/"],
    RunnerStatus::Failure
);

// commands.insert(
//     HookType::Global,
//     vec![
//         vec!["tag".to_string()],
//         vec!["webdav".to_string()],
//         vec!["help".to_string()],
//     ],
// );

// commands.insert(
//     HookType::Repository,
//     vec![
//         vec!["backup".to_string(), "src/".to_string()],
//         vec!["cat".to_string(), "tree".to_string(), "latest".to_string()],
//         vec!["config".to_string()],
//         vec!["check".to_string()],
//         vec!["copy".to_string()],
//         vec!["diff".to_string()],
//         vec!["dump".to_string()],
//         vec!["find".to_string()],
//         vec!["forget".to_string()],
//         vec!["init".to_string()],
//         vec!["key".to_string()],
//         vec!["list".to_string()],
//         vec!["ls".to_string()],
//         vec!["merge".to_string()],
//         vec!["snapshots".to_string()],
//         vec!["show-config".to_string()],
//         vec!["prune".to_string()],
//         vec!["restore".to_string()],
//         vec!["repair".to_string()],
//         vec!["repoinfo".to_string()],
//         vec!["tag".to_string()],
//         vec!["webdav".to_string()],
//     ],
// );

// commands.insert(
//     HookType::Backup,
//     vec![vec!["backup".to_string(), "src/".to_string()]],
// );

#[rstest]
#[case(vec!["backup", "src/"], "backup", BackupAction::WithoutBackup)]
#[case(vec!["cat", "tree", "latest"], "cat", BackupAction::WithBackup)]
#[case(vec!["config"], "config", BackupAction::WithoutBackup)]
#[case(vec!["completions", "bash"], "completions", BackupAction::WithoutBackup)]
#[case(vec!["check"], "check", BackupAction::WithBackup)]
// #[case(vec!["copy"], "copy", BackupAction::WithBackup)]
// #[case(vec!["diff"], "diff", BackupAction::WithBackup)]
// TODO: Does it work? Also: Docs command requires a TTY
// #[case(vec!["docs", "--help"], "docs", BackupAction::WithoutBackup)]
#[case(vec!["dump", "latest:src/lib.rs"], "dump", BackupAction::WithBackup)]
#[case(vec!["find"], "find", BackupAction::WithoutBackup)]
#[case(vec!["forget"], "forget", BackupAction::WithoutBackup)]
// TODO: Needs special handling
// #[case(vec!["init", "--dry-run"], "init", BackupAction::WithoutBackup)]
// TODO: Needs user input
// #[case(vec!["key", "add"], "key", BackupAction::WithoutBackup)]
#[case(vec!["list", "indexpacks"], "list", BackupAction::WithBackup)]
#[case(vec!["ls", "latest"], "ls", BackupAction::WithBackup)]
#[case(vec!["merge"], "merge", BackupAction::WithoutBackup)]
#[case(vec!["snapshots"], "snapshots", BackupAction::WithBackup)]
#[case(vec!["show-config"], "show-config", BackupAction::WithoutBackup)]
#[case(vec!["self-update"], "self-update", BackupAction::WithoutBackup)]
#[case(vec!["prune"], "prune", BackupAction::WithBackup)]
#[case(vec!["repoinfo"], "repoinfo", BackupAction::WithBackup)]
#[case(vec!["repair", "index"], "repair", BackupAction::WithBackup)]
#[case(vec!["restore", "latest", "tests/generated/test-restore", "--dry-run"], "restore", BackupAction::WithBackup)]
#[case(vec!["tag"], "tag", BackupAction::WithBackup)]
// TODO: Requires user input
// #[case(vec!["webdav"], "webdav", BackupAction::WithBackup)]
fn test_hooks_access_for_all_commands_passes(
    #[case] command_args: Vec<&str>,
    #[case] command_name: &str,
    #[case] backup_action: BackupAction,
    toml_fixture_dir: PathBuf,
    generated_dir: PathBuf,
) -> TestResult<()> {
    let file_name = format!("{command_name}_hooks_access_success");
    let mut output_config_file_name = file_name.clone();
    output_config_file_name.push_str(".toml");

    let hooks_config_path = generated_dir.join(&file_name);
    let output_config_path = generated_dir.join(output_config_file_name);

    load_template_replace_and_write(
        &toml_fixture_dir.join("commands_hooks_access_success.tpl"),
        "${{filename}}",
        &file_name,
        &output_config_path,
    )?;

    let log_live_path = generated_dir.join(format!("{file_name}.log"));

    let setup = setup(backup_action)?;

    run_hook_comparison(
        setup,
        hooks_config_path.clone(),
        command_args.as_slice(),
        &file_name,
        log_live_path.clone(),
        RunnerStatus::Success,
    )?;

    remove_file(output_config_path)?;

    Ok(())
}
