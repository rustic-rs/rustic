#![cfg(unix)]

use std::{
    os::{fd::OwnedFd, unix::net::UnixStream},
    process::{Command, Stdio},
};

#[test]
fn closed_stdout_pipe_exits_quietly() {
    let (reader, writer) = UnixStream::pair().expect("pipe should be created");
    drop(reader);
    let writer: OwnedFd = writer.into();

    let output = Command::new(env!("CARGO_BIN_EXE_rustic"))
        .arg("version")
        .stdout(Stdio::from(writer))
        .stderr(Stdio::piped())
        .output()
        .expect("rustic should exit after stdout is closed");

    assert!(
        output.status.success(),
        "rustic should treat a closed stdout pipe as success: {output:?}"
    );
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains("had a problem and crashed"),
        "rustic should not produce a crash report for a closed stdout pipe"
    );
}
