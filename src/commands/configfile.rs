use std::path::Path;

use anyhow::{Context, Result};
use clap::Parser;
use directories::ProjectDirs;
use merge::Merge;
use serde::Deserialize;

use crate::{repofile::SnapshotFilter, repository::RepositoryOptions};

use super::{backup, copy, forget, GlobalOpts};

#[derive(Default, Parser, Deserialize, Merge)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    #[clap(flatten, next_help_heading = "Global options")]
    pub global: GlobalOpts,

    #[clap(flatten, next_help_heading = "Repository options")]
    pub repository: RepositoryOptions,

    #[clap(flatten, next_help_heading = "Snapshot filter options")]
    pub snapshot_filter: SnapshotFilter,

    #[clap(skip)]
    pub backup: backup::Opts,

    #[clap(skip)]
    pub copy: copy::Targets,

    #[clap(skip)]
    pub forget: forget::ConfigOpts,
}

impl Config {
    pub fn merge_profile(&mut self, profile: &str) -> Result<()> {
        let mut path = match ProjectDirs::from("", "", "rustic") {
            Some(path) => path.config_dir().to_path_buf(),
            None => Path::new(".").to_path_buf(),
        };
        if !path.exists() {
            path = Path::new(".").to_path_buf();
        };
        let path = path.join(profile.to_string() + ".toml");

        if path.exists() {
            // TODO: This should be log::info! - however, the logging config
            // can be stored in the config file and is needed to initialize the logger
            eprintln!("using config {}", path.display());
            let data = std::fs::read_to_string(path).context("error reading config file")?;
            let mut config: Self =
                toml::from_str(&data).context("error reading TOML from config file")?;
            // if "use_profile" is defined in config file, merge this referenced profile first
            for profile in &config.global.use_profile.clone() {
                config.merge_profile(profile)?;
            }
            self.merge(config);
        } else {
            eprintln!("using no config file ({} doesn't exist)", path.display());
        };
        Ok(())
    }
}
