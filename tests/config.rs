//! Configuration file tests

use anyhow::Result;
use rstest::*;
use rustic_rs::RusticConfig;
use std::{fs, path::PathBuf};

/// Ensure all `configs` parse as a valid config files
#[rstest]
fn test_parse_rustic_configs_is_ok(
    #[files("config/**/*.toml")] config_path: PathBuf,
) -> Result<()> {
    let toml_string = fs::read_to_string(config_path)?;
    let _ = toml::from_str::<RusticConfig>(&toml_string)?;

    Ok(())
}

#[test]
fn test_debug_config_passes() {
    insta::assert_debug_snapshot!(RusticConfig::default());
}
