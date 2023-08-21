//! Configuration file tests

use log::LevelFilter;
use rustic_rs::RusticConfig;
use std::{error::Error, fs, path::PathBuf, str::FromStr};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn get_config_file_path() -> PathBuf {
    ["config", "full.toml"].iter().collect()
}
/// Ensure `full.toml` parses as a valid config file
#[test]
fn parse_full_toml_example() -> Result<()> {
    let output = std::process::Command::new("cargo")
        .args(["locate-project", "--workspace", "--message-format", "plain"])
        .output()?;
    let root = PathBuf::from_str(String::from_utf8(output.stdout)?.as_str())?;
    let root_dir = root.parent().unwrap();
    let config_path = root_dir.join(get_config_file_path());
    let toml_string = fs::read_to_string(config_path)?;
    let config: RusticConfig = toml::from_str(&toml_string)?;

    assert_eq!(
        LevelFilter::from_str(config.global.log_level.unwrap().as_str())?,
        LevelFilter::Info
    );
    assert!(!config.global.dry_run);

    Ok(())
}
