//! Completions test: runs the application as a subprocess and asserts its
//! output for the `completions` command

// #![forbid(unsafe_code)]
// #![warn(
//     missing_docs,
//     rust_2018_idioms,
//     trivial_casts,
//     unused_lifetimes,
//     unused_qualifications
// )]

use std::{
    io::{Read, Write},
    path::PathBuf,
};

use once_cell::sync::Lazy;

use abscissa_core::testing::prelude::*;

use rustic_testing::{files_differ, get_temp_file, TestResult};

// Storing this value as a [`Lazy`] static ensures that all instances of
/// the runner acquire a mutex when executing commands and inspecting
/// exit statuses, serializing what would otherwise be multithreaded
/// invocations as `cargo test` executes tests in parallel by default.
pub static LAZY_RUNNER: Lazy<CmdRunner> = Lazy::new(|| {
    let mut runner = CmdRunner::new(env!("CARGO_BIN_EXE_rustic"));
    runner.exclusive().capture_stdout();
    runner
});

fn cmd_runner() -> CmdRunner {
    LAZY_RUNNER.clone()
}

fn fixtures_dir() -> PathBuf {
    ["tests", "completions-fixtures"].iter().collect()
}

#[test]
#[ignore = "breaking changes, run before releasing"]
fn test_bash_completions_passes() -> TestResult<()> {
    let fixture_path = fixtures_dir().join("bash.txt");
    let mut file = get_temp_file()?;

    {
        let file = file.as_file_mut();
        let mut runner = cmd_runner();
        let mut cmd = runner.args(["completions", "bash"]).run();

        let mut output = String::new();
        cmd.stdout().read_to_string(&mut output)?;
        file.write_all(output.as_bytes())?;
        file.sync_all()?;
        cmd.wait()?.expect_success();
    }

    if files_differ(fixture_path, file.path())? {
        panic!("generated completions for bash shell differ, breaking change!");
    }

    Ok(())
}

#[test]
#[ignore = "breaking changes, run before releasing"]
fn test_fish_completions_passes() -> TestResult<()> {
    let fixture_path = fixtures_dir().join("fish.txt");
    let mut file = get_temp_file()?;

    {
        let file = file.as_file_mut();
        let mut runner = cmd_runner();
        let mut cmd = runner.args(["completions", "fish"]).run();

        let mut output = String::new();
        cmd.stdout().read_to_string(&mut output)?;
        file.write_all(output.as_bytes())?;
        file.sync_all()?;
        cmd.wait()?.expect_success();
    }

    if files_differ(fixture_path, file.path())? {
        panic!("generated completions for fish shell differ, breaking change!");
    }

    Ok(())
}

#[test]
#[ignore = "breaking changes, run before releasing"]
fn test_zsh_completions_passes() -> TestResult<()> {
    let fixture_path = fixtures_dir().join("zsh.txt");
    let mut file = get_temp_file()?;

    {
        let file = file.as_file_mut();
        let mut runner = cmd_runner();
        let mut cmd = runner.args(["completions", "zsh"]).run();

        let mut output = String::new();
        cmd.stdout().read_to_string(&mut output)?;
        file.write_all(output.as_bytes())?;
        file.sync_all()?;
        cmd.wait()?.expect_success();
    }

    if files_differ(fixture_path, file.path())? {
        panic!("generated completions for zsh shell differ, breaking change!");
    }

    Ok(())
}

#[test]
#[ignore = "breaking changes, run before releasing"]
fn test_powershell_completions_passes() -> TestResult<()> {
    let fixture_path = fixtures_dir().join("powershell.txt");
    let mut file = get_temp_file()?;

    {
        let file = file.as_file_mut();
        let mut runner = cmd_runner();
        let mut cmd = runner.args(["completions", "powershell"]).run();

        let mut output = String::new();
        cmd.stdout().read_to_string(&mut output)?;
        file.write_all(output.as_bytes())?;
        file.sync_all()?;
        cmd.wait()?.expect_success();
    }

    if files_differ(fixture_path, file.path())? {
        panic!("generated completions for powershell differ, breaking change!");
    }

    Ok(())
}
