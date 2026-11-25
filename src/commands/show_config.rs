//! `show-config` subcommand

use crate::{Application, RUSTIC_APP, status_err};

use abscissa_core::{Command, Runnable, Shutdown};
use anyhow::Result;
use toml::{Value, to_string_pretty};

/// Placeholder which replaces secrets in the output.
const REDACTED: &str = "**redacted**";

/// Keys whose string values are secrets and are redacted by default.
const SECRET_KEYS: &[&str] = &["password", "prometheus-pass"];

/// Keys whose string values are URLs which may contain credentials.
const URL_KEYS: &[&str] = &["repository", "repo-hot"];

/// `show-config` subcommand
#[derive(clap::Parser, Command, Debug)]
pub(crate) struct ShowConfigCmd {
    /// Don't redact secrets (e.g. passwords) in the output
    #[clap(long)]
    no_redact: bool,
}

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
        let config = if self.no_redact {
            to_string_pretty(RUSTIC_APP.config().as_ref())?
        } else {
            let mut value = Value::try_from(RUSTIC_APP.config().as_ref())?;
            redact_secrets(&mut value);
            to_string_pretty(&value)?
        };
        println!("{config}");
        Ok(())
    }
}

/// Recursively replace secrets within a config value by `REDACTED`.
fn redact_secrets(value: &mut Value) {
    let Value::Table(table) = value else {
        return;
    };

    for (key, val) in table.iter_mut() {
        if SECRET_KEYS.contains(&key.as_str()) && val.is_str() {
            *val = Value::String(REDACTED.to_owned());
        } else if URL_KEYS.contains(&key.as_str()) && val.is_str() {
            *val = Value::String(redact_url_password(val.as_str().unwrap_or_default()));
        } else {
            redact_secrets(val);
        }
    }
}

/// Replace the password portion of the userinfo part of an URL, if any.
///
/// If the URL contains no userinfo with a password, it is returned unchanged.
fn redact_url_password(url: &str) -> String {
    let Some(scheme_sep) = url.find("://") else {
        return url.to_owned();
    };

    let (prefix, rest) = url.split_at(scheme_sep + "://".len());
    // the userinfo part, if present, ends before the first "/"
    let authority_end = rest.find('/').unwrap_or(rest.len());
    let Some(at) = rest[..authority_end].rfind('@') else {
        return url.to_owned();
    };

    let userinfo = &rest[..at];
    let Some(colon) = userinfo.find(':') else {
        return url.to_owned();
    };

    format!(
        "{prefix}{}:{REDACTED}@{}",
        &userinfo[..colon],
        &rest[at + 1..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_url_password() {
        // no scheme or no userinfo: unchanged
        assert_eq!(redact_url_password("/path/to/repo"), "/path/to/repo");
        assert_eq!(
            redact_url_password("rest:https://user:pass@host:8000/repo"),
            format!("rest:https://user:{REDACTED}@host:8000/repo")
        );
        assert_eq!(
            redact_url_password("https://user:pass@host"),
            format!("https://user:{REDACTED}@host")
        );
        // username without password: unchanged
        assert_eq!(
            redact_url_password("https://token@host"),
            "https://token@host"
        );
        // "@" within the path is not userinfo: unchanged
        assert_eq!(
            redact_url_password("https://host:8080/p@th"),
            "https://host:8080/p@th"
        );
    }

    #[test]
    fn test_redact_secrets() {
        let mut value: Value = toml::from_str(
            r#"
[global]
prometheus-pass = "prometheus-secret"

[repository]
password = "repo-secret"
password-file = "/path/to/file"
repository = "rest:https://user:pass@host/repo"
repo-hot = "/path/to/hot"
"#,
        )
        .unwrap();

        redact_secrets(&mut value);

        assert_eq!(value["global"]["prometheus-pass"].as_str(), Some(REDACTED));
        assert_eq!(value["repository"]["password"].as_str(), Some(REDACTED));
        // non-secret values are unchanged
        assert_eq!(
            value["repository"]["password-file"].as_str(),
            Some("/path/to/file")
        );
        assert_eq!(
            value["repository"]["repo-hot"].as_str(),
            Some("/path/to/hot")
        );
        // only the password portion of the URL is redacted
        assert_eq!(
            value["repository"]["repository"].as_str(),
            Some(format!("rest:https://user:{REDACTED}@host/repo").as_str())
        );
    }
}
