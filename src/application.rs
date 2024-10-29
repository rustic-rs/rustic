//! Rustic Abscissa Application
use std::{env, process};

use abscissa_core::{
    application::{self, fatal_error, AppCell},
    config::{self, CfgCell},
    path::{AbsPath, AbsPathBuf, ExePath, RootPath, SecretsPath},
    terminal::component::Terminal,
    trace::{self, Tracing},
    Application, Component, FrameworkError, FrameworkErrorKind, Shutdown,
};
use anyhow::Result;
use directories::ProjectDirs;

// use crate::helpers::*;
use crate::{commands::EntryPoint, config::RusticConfig};

/// Application state
pub static RUSTIC_APP: AppCell<RusticApp> = AppCell::new();

// Constants
pub mod constants {
    pub const RUSTIC_DOCS_URL: &str = "https://rustic.cli.rs/docs";
    pub const RUSTIC_DEV_DOCS_URL: &str = "https://rustic.cli.rs/dev-docs";
    pub const RUSTIC_CONFIG_DOCS_URL: &str =
        "https://github.com/rustic-rs/rustic/blob/main/config/README.md";
    /// Name of the application's secrets directory
    pub(crate) const SECRETS_DIR: &str = "secrets";
    pub(crate) const LOGS_DIR: &str = "logs";
}
/// Rustic Application
#[derive(Debug)]
pub struct RusticApp {
    /// Application configuration.
    config: CfgCell<RusticConfig>,

    /// Application state.
    state: application::State<Self>,
}

#[derive(Clone, Debug)]
pub struct RusticPaths {
    /// Path to the application's executable.
    exe: AbsPathBuf,

    /// Path to the application's root directory
    root: AbsPathBuf,

    /// Path to the application's secrets
    secrets: AbsPathBuf,

    /// Path to the application's cache directory
    cache: AbsPathBuf,

    /// Path to the application's log directory
    logs: AbsPathBuf,

    /// Path to the application's configuration directory
    config: AbsPathBuf,
}

impl ExePath for RusticPaths {
    fn exe(&self) -> &AbsPath {
        self.exe.as_ref()
    }
}

impl RootPath for RusticPaths {
    fn root(&self) -> &AbsPath {
        self.root.as_ref()
    }
}

impl SecretsPath for RusticPaths {
    fn secrets(&self) -> &AbsPath {
        self.secrets.as_ref()
    }
}

impl RusticPaths {
    fn from_project_dirs() -> Result<Self, FrameworkError> {
        let project_dirs = ProjectDirs::from("", "", "rustic").ok_or_else(|| {
            FrameworkErrorKind::PathError {
                name: Some("project_dirs".into()),
            }
            .context("failed to determine project directories")
        })?;

        let exe = canonical_path::current_exe()?;
        let root = canonical_path::CanonicalPathBuf::new(project_dirs.data_dir())?;
        let secrets = root.join(constants::SECRETS_DIR)?;
        let logs = root.join(constants::LOGS_DIR)?;
        let config = canonical_path::CanonicalPathBuf::new(project_dirs.config_dir())?;
        let cache = canonical_path::CanonicalPathBuf::new(project_dirs.cache_dir())?;

        Ok(Self {
            exe,
            root,
            secrets,
            logs,
            cache,
            config,
        })
    }
}

impl Default for RusticPaths {
    fn default() -> Self {
        Self::from_project_dirs().expect("failed to determine project directories")
    }
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
    type Paths = RusticPaths;

    /// Accessor for application configuration.
    fn config(&self) -> config::Reader<RusticConfig> {
        self.config.read()
    }

    /// Borrow the application state immutably.
    fn state(&self) -> &application::State<Self> {
        &self.state
    }

    /// Initialize the framework's default set of components, potentially
    /// sourcing terminal and tracing options from command line arguments.
    fn framework_components(
        &mut self,
        command: &Self::Cmd,
    ) -> Result<Vec<Box<dyn Component<Self>>>, FrameworkError> {
        let terminal = Terminal::new(self.term_colors(command));
        let tracing = Tracing::new(self.tracing_config(command), self.term_colors(command))
            .expect("tracing subsystem failed to initialize");

        Ok(vec![Box::new(terminal), Box::new(tracing)])
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

        let global_hooks = config.global.hooks.clone();
        self.config.set_once(config);

        global_hooks.run_before().map_err(|err| -> FrameworkError {
            FrameworkErrorKind::ProcessError.context(err).into()
        })?;

        Ok(())
    }

    /// Shut down this application gracefully
    fn shutdown(&self, shutdown: Shutdown) -> ! {
        let exit_code = match shutdown {
            Shutdown::Crash => 1,
            _ => 0,
        };
        self.shutdown_with_exitcode(shutdown, exit_code)
    }

    /// Shut down this application gracefully, exiting with given exit code.
    fn shutdown_with_exitcode(&self, shutdown: Shutdown, exit_code: i32) -> ! {
        let hooks = &RUSTIC_APP.config().global.hooks;
        match shutdown {
            Shutdown::Crash => _ = hooks.run_failed(),
            _ => _ = hooks.run_after(),
        };
        _ = hooks.run_finally();
        let result = self.state().components().shutdown(self, shutdown);
        if let Err(e) = result {
            fatal_error(self, &e)
        }

        process::exit(exit_code);
    }

    /// Get tracing configuration from command-line options
    fn tracing_config(&self, command: &EntryPoint) -> trace::Config {
        if command.verbose {
            trace::Config::verbose()
        } else {
            command
                .config
                .global
                .log_level
                .as_ref()
                .map_or_else(trace::Config::default, |level| {
                    trace::Config::from(level.to_owned())
                })
        }
    }
}
