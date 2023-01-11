use std::path::Path;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use merge::Merge;
use serde::Deserialize;
use toml::Value;

pub struct RusticConfig(Value);

impl RusticConfig {
    pub fn new(profile: &str) -> Result<Self> {
        let mut path = match ProjectDirs::from("", "", "rustic") {
            Some(path) => path.config_dir().to_path_buf(),
            None => Path::new(".").to_path_buf(),
        };
        if !path.exists() {
            path = Path::new(".").to_path_buf();
        };
        let path = path.join(profile.to_string() + ".toml");

        let config = if path.exists() {
            // TODO: This should be log::info! - however, the logging config
            // can be stored in the config file and is needed to initialize the logger
            eprintln!("using config {}", path.display());
            let data = std::fs::read_to_string(path).context("error reading config file")?;
            toml::from_str(&data).context("error reading TOML from config file")?
        } else {
            eprintln!("using no config file ({} doesn't exist)", path.display());
            Value::Array(Vec::new())
        };

        Ok(RusticConfig(config))
    }

    fn get_value(&self, section: &str) -> Option<&Value> {
        // loop over subsections separated by '.'
        section
            .split('.')
            .fold(Some(&self.0), |acc, x| acc.and_then(|value| value.get(x)))
    }

    pub fn merge_into<'de, Opts>(&self, section: &str, opts: &mut Opts) -> Result<()>
    where
        Opts: Merge + Deserialize<'de>,
    {
        if let Some(value) = self.get_value(section) {
            let config: Opts = value.clone().try_into()?;
            opts.merge(config);
        }
        Ok(())
    }

    pub fn get<'de, Opts>(&self, section: &str) -> Result<Opts>
    where
        Opts: Default + Deserialize<'de>,
    {
        match self.get_value(section) {
            Some(value) => Ok(value.clone().try_into()?),
            None => Ok(Opts::default()),
        }
    }
}
