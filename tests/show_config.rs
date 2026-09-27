use assert_cmd::Command;
use rustic_testing::TestResult;
use tempfile::tempdir;

#[test]
fn show_config_redacts_profile_and_url_passwords() -> TestResult<()> {
    let temp_dir = tempdir()?;
    let profile = temp_dir.path().join("secrets.toml");
    std::fs::write(
        &profile,
        "[repository]\npassword = \"profile-secret\"\nrepository = \"rest:https://alice:url-secret@example.invalid/repo\"\n",
    )?;

    let output = Command::new(env!("CARGO_BIN_EXE_rustic"))
        .arg("-P")
        .arg(&profile)
        .arg("show-config")
        .output()?;
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout)?;
    assert!(!stdout.contains("profile-secret"), "{stdout}");
    assert!(!stdout.contains("url-secret"), "{stdout}");
    assert!(stdout.contains("password = \"redacted\""), "{stdout}");
    assert!(
        stdout.contains("rest:https://alice:redacted@example.invalid/repo"),
        "{stdout}"
    );

    Ok(())
}
