//! Config validation tests for `check-config`.

use std::{env, fs, path::Path};

use assert_cmd::Command;
use predicates::prelude::predicate;
use rustic_testing::TestResult;
use tempfile::tempdir;

fn rustic_cmd() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rustic"));
    for (key, _) in env::vars_os() {
        let key_string = key.to_string_lossy();
        let remove = key_string.starts_with("RUSTIC_")
            || key_string.starts_with("OPENDAL")
            || key_string.starts_with("OTEL_");
        if remove {
            cmd.env_remove(&key);
        }
    }
    cmd
}

fn manifest_path(path: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(path)
        .display()
        .to_string()
}

#[test]
fn check_config_passes_valid_examples() -> TestResult<()> {
    for profile in [
        "config/local.toml",
        "config/services/s3_aws.toml",
        "config/services/sftp.toml",
        "config/services/rclone_ovh-hot-cold.toml",
    ] {
        rustic_cmd()
            .args(["-P", &manifest_path(profile), "check-config"])
            .assert()
            .success()
            .stdout(predicate::str::contains("config ok"));
    }

    Ok(())
}

#[test]
fn check_config_fails_invalid_backup_shape() -> TestResult<()> {
    let temp_dir = tempdir()?;
    fs::write(
        temp_dir.path().join("bad.toml"),
        r#"
[repository]
repository = "/tmp/rustic-repo"

[backup]
sources = ["/home"]
"#,
    )?;

    rustic_cmd()
        .current_dir(temp_dir.path())
        .args(["-P", "bad", "check-config"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "key \"sources\" is not valid in the [backup] section",
        ));

    Ok(())
}
