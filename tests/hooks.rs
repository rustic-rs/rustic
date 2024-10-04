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

use std::{collections::BTreeMap, path::PathBuf};

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
    hook_fixture_dir().join("toml")
}

#[fixture]
fn log_fixture_dir() -> PathBuf {
    hook_fixture_dir().join("log")
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum HookType {
    Global,
    Repository,
    Backup,
}

#[fixture]
fn commands_fixture() -> BTreeMap<HookType, Vec<Vec<String>>> {
    let mut commands = BTreeMap::new();

    commands.insert(
        HookType::Global,
        vec![
            // vec!["backup".to_string(), "src/".to_string()],
            // vec!["cat".to_string(), "tree".to_string(), "latest".to_string()],
            // vec!["config".to_string()],
            // vec!["completions".to_string(), "bash".to_string()],
            // vec!["check".to_string()],
            vec!["backup".to_string()],
            vec!["cat".to_string()],
            vec!["config".to_string()],
            vec!["completions".to_string()],
            vec!["check".to_string()],
            // TODO: Fix command invocation for testing
            vec!["copy".to_string()],
            // TODO: Fix command invocation for testing
            vec!["diff".to_string()],
            // Can't test docs command as it requires a TTY
            vec!["docs".to_string()],
            // TODO: Fix command invocation for testing
            vec!["dump".to_string()],
            // TODO: Fix command invocation for testing
            vec!["find".to_string()],
            vec!["forget".to_string()],
            // TODO: Fix command invocation for testing
            vec!["init".to_string()],
            vec!["key".to_string()],
            vec!["list".to_string()],
            vec!["ls".to_string()],
            vec!["merge".to_string()],
            vec!["snapshots".to_string()],
            vec!["show-config".to_string()],
            vec!["self-update".to_string()],
            vec!["prune".to_string()],
            vec!["restore".to_string()],
            vec!["repair".to_string()],
            vec!["repoinfo".to_string()],
            vec!["tag".to_string()],
            vec!["webdav".to_string()],
            vec!["help".to_string()],
        ],
    );

    commands.insert(
        HookType::Repository,
        vec![
            vec!["backup".to_string(), "src/".to_string()],
            vec!["cat".to_string(), "tree".to_string(), "latest".to_string()],
            vec!["config".to_string()],
            vec!["check".to_string()],
            vec!["copy".to_string()],
            vec!["diff".to_string()],
            vec!["dump".to_string()],
            vec!["find".to_string()],
            vec!["forget".to_string()],
            vec!["init".to_string()],
            vec!["key".to_string()],
            vec!["list".to_string()],
            vec!["ls".to_string()],
            vec!["merge".to_string()],
            vec!["snapshots".to_string()],
            vec!["show-config".to_string()],
            vec!["prune".to_string()],
            vec!["restore".to_string()],
            vec!["repair".to_string()],
            vec!["repoinfo".to_string()],
            vec!["tag".to_string()],
            vec!["webdav".to_string()],
        ],
    );

    commands.insert(
        HookType::Backup,
        vec![vec!["backup".to_string(), "src/".to_string()]],
    );

    commands
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

fn setup() -> TestResult<TempDir> {
    let temp_dir = tempdir()?;
    rustic_runner(&temp_dir)?
        .args(["init"])
        .assert()
        .success()
        .stderr(predicate::str::contains("successfully created."))
        .stderr(predicate::str::contains("successfully added."));

    Ok(temp_dir)
}

/// Compare the content of the repo hook log with our fixture
fn compare_logs(log_fixture_path: PathBuf, log_live_path: PathBuf) -> TestResult<()> {
    let log_fixture_content = std::fs::read_to_string(log_fixture_path)?;

    // Remove carriage returns from the fixture content on Windows
    #[cfg(windows)]
    let log_fixture_content = log_fixture_content.replace('\r', "");

    let log_live = std::fs::read_to_string(&log_live_path)?;

    remove_file(log_live_path)?;
    assert_eq!(log_fixture_content, log_live);
    Ok(())
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
    log_fixture_path: PathBuf,
    log_live_path: PathBuf,
    status: RunnerStatus,
) -> TestResult<()> {
    {
        let runner = rustic_runner(&temp_dir)?
            .args(["-P", hooks_config.to_str().unwrap()])
            .args(args)
            .assert();

        match status {
            RunnerStatus::Success => runner.success(),
            RunnerStatus::Failure => runner.failure(),
        };
    }

    compare_logs(log_fixture_path, log_live_path)?;

    Ok(())
}

#[rstest]
fn test_empty_hooks_do_nothing_passes(toml_fixture_dir: PathBuf) -> TestResult<()> {
    let hooks_config = toml_fixture_dir.join("empty_hooks_success");

    let temp_dir = setup()?;

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
        fn $name(
            toml_fixture_dir: PathBuf,
            generated_dir: PathBuf,
            log_fixture_dir: PathBuf,
        ) -> TestResult<()> {
            let hooks_config_path = toml_fixture_dir.join($fixture);
            let args = $args;

            let file_name = format!("{}.log", $fixture);
            let log_live_path = generated_dir.join(&file_name);
            let log_fixture_path = log_fixture_dir.join(file_name);

            run_hook_comparison(
                setup()?,
                hooks_config_path,
                args,
                log_fixture_path,
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

// #[rstest]
// fn test_global_hooks_for_all_commands_passes(
//     toml_fixture_dir: PathBuf,
//     generated_dir: PathBuf,
//     log_fixture_dir: PathBuf,
//     commands_fixture: BTreeMap<HookType, Vec<Vec<String>>>,
// ) -> TestResult<()> {
//     let hooks_config_path = toml_fixture_dir.join("global_hooks_success");
//     let file_name = "global_hooks_success.log";
//     let log_live_path = generated_dir.join(file_name);
//     let log_fixture_path = log_fixture_dir.join(file_name);

//     if let Some((_, commands)) = commands_fixture.get_key_value(&HookType::Global) {
//         for command in commands.iter() {
//             let mut args = command.iter().map(|s| s.as_str()).collect::<Vec<&str>>();

//             // We use the help command of each command to test the global hooks
//             args.push("help");

//             run_hook_comparison(
//                 setup()?,
//                 hooks_config_path.clone(),
//                 args.as_slice(),
//                 log_fixture_path.clone(),
//                 log_live_path.clone(),
//                 RunnerStatus::Success,
//             )?;
//         }
//     }

//     Ok(())
// }
