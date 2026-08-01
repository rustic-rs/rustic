//! Config profile test: runs the application as a subprocess and asserts its
//! output for the `show-config` command

use assert_cmd::Command;
use insta::assert_snapshot;
use rustic_testing::TestResult;
use tempfile::tempdir;

#[test]
fn test_show_config_passes() -> TestResult<()> {
    let config_root = tempdir()?;
    let mut command = Command::new(env!("CARGO_BIN_EXE_rustic"));
    let output = command
        .arg("show-config")
        .env("HOME", config_root.path())
        .env("RUSTIC_HOME", config_root.path().join("rustic-home"))
        .env("XDG_CONFIG_HOME", config_root.path().join("xdg-home"))
        .env("XDG_CONFIG_DIRS", config_root.path().join("xdg-dirs"))
        .env_remove("RUSTIC_USE_PROFILE")
        .output()?;

    assert!(
        output.status.success(),
        "show-config failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert_snapshot!(String::from_utf8(output.stdout)?);

    Ok(())
}
