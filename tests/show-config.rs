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

use std::{io::Read, sync::LazyLock};

use abscissa_core::testing::prelude::*;

use rustic_testing::TestResult;

// Storing this value as a [`Lazy`] static ensures that all instances of
// the runner acquire a mutex when executing commands and inspecting
// exit statuses, serializing what would otherwise be multithreaded
// invocations as `cargo test` executes tests in parallel by default.
pub static LAZY_RUNNER: LazyLock<CmdRunner> = LazyLock::new(|| {
    let mut runner = CmdRunner::new(env!("CARGO_BIN_EXE_rustic"));
    runner.exclusive().capture_stdout();
    runner
});

fn cmd_runner() -> CmdRunner {
    LAZY_RUNNER.clone()
}

#[test]
fn test_show_config_passes() -> TestResult<()> {
    let mut output = String::new();

    {
        _ = cmd_runner()
            .args(["show-config"])
            .run()
            .stdout()
            .read_to_string(&mut output)?;
    }

    // remove the first three lines of the output
    output = output.lines().skip(3).collect::<Vec<&str>>().join("\n");

    insta::assert_snapshot!(output);

    Ok(())
}
