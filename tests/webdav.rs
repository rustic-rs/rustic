//! Regression coverage for the WebDAV command.

#[cfg(feature = "webdav")]
mod webdav {
    use std::net::TcpListener;

    use assert_cmd::Command;
    use rustic_testing::TestResult;
    use tempfile::{tempdir, TempDir};

    fn rustic_runner(temp_dir: &TempDir) -> Command {
        let mut runner = Command::new(env!("CARGO_BIN_EXE_rustic"));
        runner
            .arg("--repository")
            .arg(temp_dir.path().join("repo"))
            .args(["--password", "test", "--no-progress"]);
        runner
    }

    fn setup() -> TestResult<TempDir> {
        let temp_dir = tempdir()?;
        rustic_runner(&temp_dir).arg("init").assert().success();
        Ok(temp_dir)
    }

    #[test]
    fn occupied_address_is_reported_without_a_panic() -> TestResult<()> {
        let temp_dir = setup()?;
        let _listener = TcpListener::bind("127.0.0.1:0")?;
        let address = _listener.local_addr()?.to_string();

        let output = rustic_runner(&temp_dir)
            .arg("webdav")
            .arg("--address")
            .arg(&address)
            .output()?;
        let output_text = format!(
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        assert!(
            !output.status.success(),
            "webdav unexpectedly succeeded while {address} was occupied:\n{output_text}"
        );
        assert!(
            output_text
                .to_ascii_lowercase()
                .contains("address already in use"),
            "webdav did not report the bind failure:\n{output_text}"
        );
        assert!(
            !output_text.to_ascii_lowercase().contains("panic"),
            "webdav panicked instead of reporting the bind failure:\n{output_text}"
        );

        Ok(())
    }
}
