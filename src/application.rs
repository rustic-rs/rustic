//! Rustic Abscissa Application
use std::fs::File;
use std::str::FromStr;

use abscissa_core::{
    application::{self, AppCell},
    config::{self, CfgCell},
    terminal::{component::Terminal, ColorChoice},
    Application, Component, FrameworkError, FrameworkErrorKind, StandardPaths,
};

use anyhow::Result;
use simplelog::{CombinedLogger, LevelFilter, TermLogger, TerminalMode, WriteLogger};

// use crate::helpers::*;
use crate::{commands::EntryPoint, config::RusticConfig};

/// Application state
pub static RUSTIC_APP: AppCell<RusticApp> = AppCell::new();

/// Rustic Application
#[derive(Debug)]
pub struct RusticApp {
    /// Application configuration.
    config: CfgCell<RusticConfig>,

    /// Application state.
    state: application::State<Self>,
}

/// Initialize a new application instance.
///
/// By default no configuration is loaded, and the framework state is
/// initialized to a default, empty state (no components, threads, etc).
impl Default for RusticApp {
    fn default() -> Self {
        Self {
            config: CfgCell::default(),
            state: application::State::default(),
        }
    }
}

impl Application for RusticApp {
    /// Entrypoint command for this application.
    type Cmd = EntryPoint;

    /// Application configuration.
    type Cfg = RusticConfig;

    /// Paths to resources within the application.
    type Paths = StandardPaths;

    /// Accessor for application configuration.
    fn config(&self) -> config::Reader<RusticConfig> {
        self.config.read()
    }

    /// Borrow the application state immutably.
    fn state(&self) -> &application::State<Self> {
        &self.state
    }

    /// Returns the framework components used by this application.
    fn framework_components(
        &mut self,
        command: &Self::Cmd,
    ) -> Result<Vec<Box<dyn Component<Self>>>, FrameworkError> {
        // we only ue the terminal component
        let terminal = Terminal::new(self.term_colors(command));

        Ok(vec![Box::new(terminal)])
    }

    /// Register all components used by this application.
    ///
    /// If you would like to add additional components to your application
    /// beyond the default ones provided by the framework, this is the place
    /// to do so.
    fn register_components(&mut self, command: &Self::Cmd) -> Result<(), FrameworkError> {
        let framework_components = self.framework_components(command)?;
        let mut app_components = self.state.components_mut();
        app_components.register(framework_components)
    }

    /// Post-configuration lifecycle callback.
    ///
    /// Called regardless of whether config is loaded to indicate this is the
    /// time in app lifecycle when configuration would be loaded if
    /// possible.
    fn after_config(&mut self, config: Self::Cfg) -> Result<(), FrameworkError> {
        // Configure components
        self.state.components_mut().after_config(&config)?;

        // start logger
        let level_filter = match &config.global.log_level {
            Some(level) => LevelFilter::from_str(level)
                .map_err(|e| FrameworkErrorKind::ConfigError.context(e))?,
            None => LevelFilter::Info,
        };
        match &config.global.log_file {
            None => TermLogger::init(
                level_filter,
                simplelog::ConfigBuilder::new()
                    .set_time_level(LevelFilter::Off)
                    .build(),
                TerminalMode::Stderr,
                ColorChoice::Auto,
            )
            .map_err(|e| FrameworkErrorKind::ConfigError.context(e))?,

            Some(file) => CombinedLogger::init(vec![
                TermLogger::new(
                    level_filter.max(LevelFilter::Warn),
                    simplelog::ConfigBuilder::new()
                        .set_time_level(LevelFilter::Off)
                        .build(),
                    TerminalMode::Stderr,
                    ColorChoice::Auto,
                ),
                WriteLogger::new(
                    level_filter,
                    simplelog::Config::default(),
                    File::options().create(true).append(true).open(file)?,
                ),
            ])
            .map_err(|e| FrameworkErrorKind::ConfigError.context(e))?,
        }

        self.config.set_once(config);

        Ok(())
    }
}
