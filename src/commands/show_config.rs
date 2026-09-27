//! `show-config` subcommand

use crate::{Application, RUSTIC_APP, config::RusticConfig, status_err};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use toml::to_string_pretty;

/// `show-config` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct ShowConfigCmd {}

impl Runnable for ShowConfigCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl ShowConfigCmd {
    fn inner_run(&self) -> Result<()> {
        let config = to_string_pretty(&redacted_config(RUSTIC_APP.config().as_ref()))?;
        println!("{config}");
        Ok(())
    }
}

const REDACTED: &str = "redacted";

fn redacted_config(config: &RusticConfig) -> RusticConfig {
    let mut config = config.clone();
    if config.repository.credential_opts.password.is_some() {
        config.repository.credential_opts.password = Some(REDACTED.to_owned());
    }
    redact_repository_password(&mut config.repository.be.repository);
    redact_repository_password(&mut config.repository.be.repo_hot);
    config
}

fn redact_repository_password(repository: &mut Option<String>) {
    let Some(repository) = repository else {
        return;
    };
    let Some((backend, location)) = repository.split_once(':') else {
        return;
    };
    let Ok(mut location) = reqwest::Url::parse(location) else {
        return;
    };
    if location.password().is_some() {
        let _ = location.set_password(Some(REDACTED));
        *repository = format!("{backend}:{location}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_passwords_from_displayed_config() {
        let mut config = RusticConfig::default();
        config.repository.credential_opts.password = Some("correct horse battery staple".into());
        config.repository.be.repository =
            Some("rest:https://primary:secret@example.invalid/repository".into());
        config.repository.be.repo_hot =
            Some("rest:https://hot:another-secret@example.invalid/repository".into());

        let displayed = redacted_config(&config);
        let output = to_string_pretty(&displayed).unwrap();

        assert!(!output.contains("correct horse battery staple"));
        assert!(!output.contains("secret"));
        assert!(!output.contains("another-secret"));
        assert!(output.contains("password = \"redacted\""));
        assert!(output.contains("rest:https://primary:redacted@example.invalid/repository"));
        assert!(output.contains("rest:https://hot:redacted@example.invalid/repository"));

        assert_eq!(
            config.repository.credential_opts.password.as_deref(),
            Some("correct horse battery staple")
        );
    }
}
