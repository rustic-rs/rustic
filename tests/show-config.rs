//! Config profile test: runs the application as a subprocess and asserts its
//! output for the `show-config` command

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
// the runner acquire a mutex when executing commands and inspecting
// exit statuses, serializing what would otherwise be multithreaded
// invocations as `cargo test` executes tests in parallel by default.
pub static LAZY_RUNNER: Lazy<CmdRunner> = Lazy::new(|| {
    let mut runner = CmdRunner::new(env!("CARGO_BIN_EXE_rustic"));
    runner.exclusive().capture_stdout();
    runner
});

fn cmd_runner() -> CmdRunner {
    LAZY_RUNNER.clone()
}

fn fixture() -> PathBuf {
    ["tests", "show-config-fixtures", "empty.txt"]
        .iter()
        .collect()
}

#[test]
fn show_config_passes() -> TestResult<()> {
    let fixture_file = fixture();
    let mut file = get_temp_file()?;

    {
        let file = file.as_file_mut();
        let mut runner = cmd_runner();
        let mut cmd = runner.args(["show-config"]).run();

        let mut output = String::new();
        cmd.stdout().read_to_string(&mut output)?;
        file.write_all(output.as_bytes())?;
        file.sync_all()?;
        cmd.wait()?.expect_success();
    }

    if files_differ(fixture_file, file.path())? {
        panic!("generated empty.txt differs, breaking change!");
    }

    Ok(())
}
