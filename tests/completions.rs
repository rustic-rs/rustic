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

use std::{io::Read, sync::LazyLock};

use abscissa_core::testing::prelude::*;
use insta::assert_snapshot;
use rstest::rstest;

use rustic_testing::TestResult;

// Storing this value as a [`Lazy`] static ensures that all instances of
/// the runner acquire a mutex when executing commands and inspecting
/// exit statuses, serializing what would otherwise be multithreaded
/// invocations as `cargo test` executes tests in parallel by default.
pub static LAZY_RUNNER: LazyLock<CmdRunner> = LazyLock::new(|| {
    let mut runner = CmdRunner::new(env!("CARGO_BIN_EXE_rustic"));
    runner.exclusive().capture_stdout();
    runner
});

fn cmd_runner() -> CmdRunner {
    LAZY_RUNNER.clone()
}

#[rstest]
#[case("bash")]
#[case("fish")]
#[case("zsh")]
#[case("powershell")]
fn test_completions_passes(#[case] shell: &str) -> TestResult<()> {
    let mut runner = cmd_runner();

    let mut cmd = runner.args(["completions", shell]).run();

    let mut output = String::new();

    cmd.stdout().read_to_string(&mut output)?;

    #[cfg(target_os = "windows")]
    let os = "windows";
    #[cfg(target_os = "linux")]
    let os = "linux";
    #[cfg(target_os = "macos")]
    let os = "macos";

    let name = format!("completions-{}-{}", shell, os);

    assert_snapshot!(name, output);

    cmd.wait()?.expect_success();

    Ok(())
}
