//! `webdav` subcommand

// ignore markdown clippy lints as we use doc-comments to generate clap help texts
#![allow(clippy::doc_markdown)]

use std::net::ToSocketAddrs;

use crate::{
    Application, RUSTIC_APP, RusticConfig,
    repository::{IndexedRepo, get_filtered_snapshots},
    status_err,
};
use rustic_core::vfs::{FilePolicy, IdenticalSnapshot, Latest, Vfs};

use abscissa_core::{Command, FrameworkError, Runnable, Shutdown, config::Override};
use anyhow::{Result, anyhow};
use axum::{
    Router,
    extract::{Request, State},
    http::{HeaderValue, StatusCode, header},
    response::IntoResponse,
    routing::any,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use conflate::Merge;
use dav_server::DavHandler;
use log::{info, warn};
use serde::{Deserialize, Serialize};

mod webdavfs;
use webdavfs::WebDavFS;

#[derive(Clone, Command, Default, Debug, clap::Parser, Serialize, Deserialize, Merge)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct WebDavCmd {
    /// Address to bind the webdav server to. [default: "localhost:8000"]
    #[clap(long, value_name = "ADDRESS")]
    #[merge(strategy=conflate::option::overwrite_none)]
    address: Option<String>,

    /// The path template to use for snapshots. {id}, {id_long}, {time}, {username}, {hostname}, {label}, {tags}, {backup_start}, {backup_end} are replaced. [default: "[{hostname}]/[{label}]/{time}"]
    #[clap(long)]
    #[merge(strategy=conflate::option::overwrite_none)]
    path_template: Option<String>,

    /// The time template to use to display times in the path template. See https://pubs.opengroup.org/onlinepubs/009695399/functions/strftime.html for format options. [default: "%Y-%m-%d_%H-%M-%S"]
    #[clap(long)]
    #[merge(strategy=conflate::option::overwrite_none)]
    time_template: Option<String>,

    /// Use symlinks. This may not be supported by all WebDAV clients
    #[clap(long)]
    #[merge(strategy=conflate::bool::overwrite_false)]
    symlinks: bool,

    /// How to handle access to files. [default: "forbidden" for hot/cold repositories, else "read"]
    #[clap(long)]
    #[merge(strategy=conflate::option::overwrite_none)]
    file_access: Option<String>,

    /// User name required for WebDAV Basic Authentication
    #[clap(long, value_name = "USER", env = "RUSTIC_WEBDAV_USER")]
    #[merge(strategy = conflate::option::overwrite_none)]
    auth_user: Option<String>,

    /// Password required for WebDAV Basic Authentication
    ///
    /// # Warning
    ///
    /// * Using --auth-password can reveal the password in the process list.
    #[clap(
        long,
        value_name = "PASSWORD",
        env = "RUSTIC_WEBDAV_PASSWORD",
        hide_env_values = true
    )]
    #[merge(strategy = conflate::option::overwrite_none)]
    auth_password: Option<String>,

    /// Specify directly which snapshot/path to serve
    ///
    /// Snapshot can be identified the following ways: "01a2b3c4" or "latest" or "latest~N" (N >= 0)
    #[clap(value_name = "SNAPSHOT[:PATH]")]
    #[merge(strategy=conflate::option::overwrite_none)]
    snapshot_path: Option<String>,
}

impl Override<RusticConfig> for WebDavCmd {
    // Process the given command line options, overriding settings from
    // a configuration file using explicit flags taken from command-line
    // arguments.
    fn override_config(&self, mut config: RusticConfig) -> Result<RusticConfig, FrameworkError> {
        let mut self_config = self.clone();
        // merge "webdav" section from config file, if given
        self_config.merge(config.webdav);
        config.webdav = self_config;
        Ok(config)
    }
}

impl Runnable for WebDavCmd {
    fn run(&self) {
        let config = RUSTIC_APP.config();
        let result = WebDavAuth::from_options(
            config.webdav.auth_user.as_deref(),
            config.webdav.auth_password.as_deref(),
        )
        .and_then(|auth| {
            config
                .repository
                .run_indexed(|repo| self.inner_run(repo, auth))
        });

        if let Err(err) = result {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        };
    }
}

impl WebDavCmd {
    /// be careful about self VS RUSTIC_APP.config() usage
    /// only the RUSTIC_APP.config() involves the TOML and ENV merged configurations
    /// see https://github.com/rustic-rs/rustic/issues/1242
    fn inner_run(&self, repo: IndexedRepo, auth: Option<WebDavAuth>) -> Result<()> {
        let config = RUSTIC_APP.config();

        let path_template = config
            .webdav
            .path_template
            .clone()
            .unwrap_or_else(|| "[{hostname}]/[{label}]/{time}".to_string());
        let time_template = config
            .webdav
            .time_template
            .clone()
            .unwrap_or_else(|| "%Y-%m-%d_%H-%M-%S".to_string());

        let vfs = if let Some(snap) = &config.webdav.snapshot_path {
            let node =
                repo.node_from_snapshot_path(snap, |sn| config.snapshot_filter.matches(sn))?;
            Vfs::from_dir_node(&node)
        } else {
            let snapshots = get_filtered_snapshots(&repo)?;
            let (latest, identical) = if config.webdav.symlinks {
                (Latest::AsLink, IdenticalSnapshot::AsLink)
            } else {
                (Latest::AsDir, IdenticalSnapshot::AsDir)
            };
            Vfs::from_snapshots(snapshots, &path_template, &time_template, latest, identical)?
        };

        let addr = config
            .webdav
            .address
            .clone()
            .unwrap_or_else(|| "localhost:8000".to_string())
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| anyhow!("no address given"))?;

        let file_access = config.webdav.file_access.as_ref().map_or_else(
            || {
                if repo.config().is_hot == Some(true) {
                    Ok(FilePolicy::Forbidden)
                } else {
                    Ok(FilePolicy::Read)
                }
            },
            |s| s.parse(),
        )?;

        let webdavfs = WebDavFS::new(repo, vfs, file_access);
        let dav_server = DavHandler::builder()
            .filesystem(Box::new(webdavfs))
            .build_handler();

        if auth.is_some() {
            warn!(
                "WebDAV Basic Authentication does not encrypt credentials; use a TLS-terminating reverse proxy outside a trusted network"
            );
        }

        let app = Router::new()
            .route("/", any(handle_dav))
            .route("/{*path}", any(handle_dav))
            .with_state(WebDavState {
                dav: dav_server,
                auth,
            });

        info!("serving webdav on {addr}");
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(async {
                axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await
            })?;

        Ok(())
    }
}

#[derive(Clone)]
struct WebDavState {
    dav: DavHandler,
    auth: Option<WebDavAuth>,
}

#[derive(Clone)]
struct WebDavAuth {
    credentials: Vec<u8>,
}

impl WebDavAuth {
    fn from_options(username: Option<&str>, password: Option<&str>) -> Result<Option<Self>> {
        match (username, password) {
            (None, None) => Ok(None),
            (Some(username), Some(password)) => {
                if username.contains(':') {
                    return Err(anyhow!(
                        "WebDAV authentication username must not contain a colon"
                    ));
                }

                Ok(Some(Self {
                    credentials: format!("{username}:{password}").into_bytes(),
                }))
            }
            _ => Err(anyhow!(
                "WebDAV Basic Authentication requires both --auth-user and --auth-password"
            )),
        }
    }

    fn is_authorized(&self, authorization: Option<&HeaderValue>) -> bool {
        let Some(authorization) = authorization.and_then(|header| header.to_str().ok()) else {
            return false;
        };
        let Some((scheme, encoded_credentials)) = authorization.split_once(' ') else {
            return false;
        };
        if !scheme.eq_ignore_ascii_case("basic") {
            return false;
        }
        let Ok(credentials) = STANDARD.decode(encoded_credentials) else {
            return false;
        };

        credentials == self.credentials
    }
}

async fn handle_dav(State(state): State<WebDavState>, req: Request) -> impl IntoResponse {
    if state
        .auth
        .as_ref()
        .is_some_and(|auth| !auth.is_authorized(req.headers().get(header::AUTHORIZATION)))
    {
        return (
            StatusCode::UNAUTHORIZED,
            [(
                header::WWW_AUTHENTICATE,
                HeaderValue::from_static("Basic realm=\"rustic\""),
            )],
        )
            .into_response();
    }

    state.dav.handle(req).await.into_response()
}

#[cfg(test)]
mod tests {
    use axum::body::Body;

    use super::*;

    fn authorization(value: &str) -> HeaderValue {
        HeaderValue::from_str(value).unwrap()
    }

    #[test]
    fn webdav_auth_is_disabled_without_both_options() {
        assert!(WebDavAuth::from_options(None, None).unwrap().is_none());
        assert!(WebDavAuth::from_options(Some("user"), None).is_err());
        assert!(WebDavAuth::from_options(None, Some("password")).is_err());
    }

    #[test]
    fn webdav_auth_rejects_colons_in_usernames() {
        assert!(WebDavAuth::from_options(Some("user:name"), Some("password")).is_err());
    }

    #[test]
    fn webdav_auth_accepts_only_matching_basic_credentials() {
        let auth = WebDavAuth::from_options(Some("user"), Some("pass:word"))
            .unwrap()
            .unwrap();

        assert!(auth.is_authorized(Some(&authorization("Basic dXNlcjpwYXNzOndvcmQ="))));
        assert!(!auth.is_authorized(None));
        assert!(!auth.is_authorized(Some(&authorization("Bearer token"))));
        assert!(!auth.is_authorized(Some(&authorization("Basic malformed"))));
        assert!(!auth.is_authorized(Some(&authorization("Basic dXNlcjp3cm9uZw=="))));
    }

    #[test]
    fn webdav_handler_challenges_unauthenticated_requests() {
        let state = WebDavState {
            dav: DavHandler::builder().build_handler(),
            auth: WebDavAuth::from_options(Some("user"), Some("password")).unwrap(),
        };
        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let response = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(handle_dav(State(state), request))
            .into_response();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers()[header::WWW_AUTHENTICATE],
            "Basic realm=\"rustic\""
        );
    }
}
