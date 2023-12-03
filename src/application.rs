//! Rustic Abscissa Application
use std::env;

use abscissa_core::{
    application::{self, AppCell},
    config::{self, CfgCell},
    terminal::component::Terminal,
    Application, Component, FrameworkError, StandardPaths,
};

use anyhow::Result;

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

        // set all given environment variables
        for (env, value) in config.global.env.iter() {
            env::set_var(env, value);
        }

        self.config.set_once(config);

        Ok(())
    }
}
