//! Service-profile smoke tests that do not contact external services.

use std::{path::PathBuf, process::Command};

#[test]
fn onedrive_service_profile_maps_opendal_credentials() {
    let profile = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config/services/onedrive.toml");

    let output = Command::new(env!("CARGO_BIN_EXE_rustic"))
        .env("OPENDAL_REFRESH_TOKEN", "one-drive-refresh-token-for-test")
        .env("OPENDAL_CLIENT_ID", "one-drive-client-id-for-test")
        .env("OPENDAL_CLIENT_SECRET", "one-drive-client-secret-for-test")
        .env("RUSTIC_PASSWORD", "repository-password-for-test")
        .arg("--use-profile")
        .arg(&profile)
        .arg("show-config")
        .output()
        .expect("run rustic show-config");

    assert!(
        output.status.success(),
        "OneDrive profile did not load: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let config = String::from_utf8(output.stdout).expect("show-config output is UTF-8");
    assert!(config.contains("repository = \"opendal:onedrive\""));
    assert!(config.contains("root = \"/rustic-backups\""));
    assert!(config.contains("refresh_token = \"one-drive-refresh-token-for-test\""));
    assert!(config.contains("client_id = \"one-drive-client-id-for-test\""));
    assert!(config.contains("client_secret = \"one-drive-client-secret-for-test\""));
    assert!(config.contains("password = \"repository-password-for-test\""));
}
