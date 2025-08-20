//! rustic hooks configuration
//!
//! Hooks are commands that are executed before and after every rustic operation.
//! They can be used to run custom scripts or commands before and after a backup,
//! copy, forget, prune or other operation.
//!
//! Depending on the hook type, the command is being executed at a different point
//! in the lifecycle of the program. The following hooks are available:
//!
//! - global hooks
//! - repository hooks
//! - backup hooks
//! - specific source-related hooks

use anyhow::Result;
use conflate::Merge;
use serde::{Deserialize, Serialize};

use rustic_core::CommandInput;

#[derive(Debug, Default, Clone, Serialize, Deserialize, Merge)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct Hooks {
    /// Call this command before every rustic operation
    #[merge(strategy = conflate::vec::append)]
    pub run_before: Vec<CommandInput>,

    /// Call this command after every successful rustic operation
    #[merge(strategy = conflate::vec::append)]
    pub run_after: Vec<CommandInput>,

    /// Call this command after every failed rustic operation
    #[merge(strategy = conflate::vec::append)]
    pub run_failed: Vec<CommandInput>,

    /// Call this command after every rustic operation
    #[merge(strategy = conflate::vec::append)]
    pub run_finally: Vec<CommandInput>,

    #[serde(skip)]
    #[merge(skip)]
    pub context: String,
}

impl Hooks {
    pub fn with_context(&self, context: &str) -> Self {
        let mut hooks = self.clone();
        hooks.context = context.to_string();
        hooks
    }

    fn run_all(cmds: &[CommandInput], context: &str, what: &str) -> Result<()> {
        for cmd in cmds {
            cmd.run(context, what, None::<(&str, &str)>)?;
        }

        Ok(())
    }

    pub fn run_before(&self) -> Result<()> {
        Self::run_all(&self.run_before, &self.context, "run-before")
    }

    pub fn run_after(&self) -> Result<()> {
        Self::run_all(&self.run_after, &self.context, "run-after")
    }

    pub fn run_failed(&self) -> Result<()> {
        Self::run_all(&self.run_failed, &self.context, "run-failed")
    }

    pub fn run_finally(&self) -> Result<()> {
        Self::run_all(&self.run_finally, &self.context, "run-finally")
    }

    /// Run the given closure using the specified hooks.
    ///
    /// Note: after a failure no error handling is done for the hooks `run_failed`
    /// and `run_finally` which must run after. However, they already log a warning
    /// or error depending on the `on_failure` setting.
    pub fn use_with<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T> {
        match self.run_before() {
            Ok(()) => match f() {
                Ok(result) => match self.run_after() {
                    Ok(()) => {
                        self.run_finally()?;
                        Ok(result)
                    }
                    Err(err_after) => {
                        _ = self.run_finally();
                        Err(err_after)
                    }
                },
                Err(err_f) => {
                    _ = self.run_failed();
                    _ = self.run_finally();
                    Err(err_f)
                }
            },
            Err(err_before) => {
                _ = self.run_failed();
                _ = self.run_finally();
                Err(err_before)
            }
        }
    }
}
