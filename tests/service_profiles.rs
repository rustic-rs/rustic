//! Service-profile smoke tests that do not contact external services.

use std::{path::PathBuf, process::Command};

#[test]
fn ovh_cold_archive_profile_maps_opendal_credentials() {
    let profile = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("config/services/opendal_ovh-cold-archive.toml");

    let output = Command::new(env!("CARGO_BIN_EXE_rustic"))
        .env("OPENDAL_ACCESS_KEY_ID", "ovh-access-key-for-test")
        .env("OPENDAL_SECRET_ACCESS_KEY", "ovh-secret-key-for-test")
        .arg("--use-profile")
        .arg(&profile)
        .arg("show-config")
        .output()
        .expect("run rustic show-config");

    assert!(
        output.status.success(),
        "OVH profile did not load: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let config = String::from_utf8(output.stdout).expect("show-config output is UTF-8");
    assert!(config.contains("access_key_id = \"ovh-access-key-for-test\""));
    assert!(config.contains("secret_access_key = \"ovh-secret-key-for-test\""));
}
