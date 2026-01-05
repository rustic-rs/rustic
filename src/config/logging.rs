use std::{path::PathBuf, sync::OnceLock};

use anyhow::Result;
use clap::{Parser, ValueHint};
use conflate::Merge;
use log::LevelFilter;
use log4rs::{
    Handle,
    append::{
        console::{ConsoleAppender, Target},
        file::FileAppender,
    },
    config::{Appender, Config, Logger, Root},
    encode::pattern::PatternEncoder,
    filter::threshold::ThresholdFilter,
};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};

/// Logging Config
#[serde_as]
#[derive(Default, Debug, Parser, Clone, Deserialize, Serialize, Merge)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct LoggingOptions {
    /// Use this log level [default: info]
    #[clap(long, global = true, env = "RUSTIC_LOG_LEVEL")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[merge(strategy=conflate::option::overwrite_none)]
    pub log_level: Option<LevelFilter>,

    /// Use this log level for the log file [default: info]
    #[clap(long, global = true, env = "RUSTIC_LOG_LEVEL_LOGFILE")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[merge(strategy=conflate::option::overwrite_none)]
    pub log_level_logfile: Option<LevelFilter>,

    /// Use this log level in dry-run mode [default: info]
    #[clap(long, global = true, env = "RUSTIC_LOG_LEVEL_DRYRUN")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[merge(strategy=conflate::option::overwrite_none)]
    pub log_level_dryrun: Option<LevelFilter>,

    /// Use this log level for dependencies [default: warn]
    #[clap(long, global = true, env = "RUSTIC_LOG_LEVEL_DEPENDENCIES")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[merge(strategy=conflate::option::overwrite_none)]
    pub log_level_dependencies: Option<LevelFilter>,

    /// Write log messages to the given file (using log-level-logfile)
    #[clap(long, global = true, env = "RUSTIC_LOG_FILE", value_name = "LOGFILE", value_hint = ValueHint::FilePath)]
    #[merge(strategy=conflate::option::overwrite_none)]
    pub log_file: Option<PathBuf>,
}

impl LoggingOptions {
    pub fn config(&self, dry_run: bool) -> Result<Config> {
        let log_level = if dry_run {
            self.log_level_dryrun
        } else {
            self.log_level
        };

        let level_filter = log_level.unwrap_or(LevelFilter::Info);
        let level_filter_logfile = self.log_level_logfile.unwrap_or(LevelFilter::Info);
        let level_filter_dependencies = self.log_level_dependencies.unwrap_or(LevelFilter::Warn);

        let stdout = ConsoleAppender::builder()
            .target(Target::Stderr)
            .encoder(Box::new(PatternEncoder::new("{h([{l}])} {m}{n}")))
            .build();

        let mut root_builder = Root::builder().appender("stdout");
        let mut config_builder = Config::builder().appender(
            Appender::builder()
                .filter(Box::new(ThresholdFilter::new(level_filter)))
                .build("stdout", Box::new(stdout)),
        );

        if let Some(file) = &self.log_file {
            let file_appender = FileAppender::builder()
                .encoder(Box::new(PatternEncoder::new("{d} [{l}] - {m}{n}")))
                .build(file)?;
            root_builder = root_builder.appender("logfile");
            config_builder = config_builder.appender(
                Appender::builder()
                    .filter(Box::new(ThresholdFilter::new(level_filter_logfile)))
                    .build("logfile", Box::new(file_appender)),
            );
        }

        let root = root_builder.build(level_filter_dependencies);
        let config = config_builder
            .logger(Logger::builder().build("rustic_rs", LevelFilter::Trace))
            .logger(Logger::builder().build("rustic_core", LevelFilter::Trace))
            .logger(Logger::builder().build("rustic_backend", LevelFilter::Trace))
            .build(root)?;
        Ok(config)
    }

    pub fn start_logger(&self, dry_run: bool) -> Result<()> {
        static HANDLE: OnceLock<Handle> = OnceLock::new();

        let config = self.config(dry_run)?;
        if let Some(handle) = HANDLE.get() {
            handle.set_config(config);
        } else {
            let handle = log4rs::init_config(config)?;
            _ = HANDLE.set(handle);
        }
        Ok(())
    }
}
