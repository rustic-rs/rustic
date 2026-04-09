//! `serve` subcommand

pub(crate) mod api;

use std::net::ToSocketAddrs;

use crate::{Application, RUSTIC_APP, RusticConfig, status_err};

use abscissa_core::{Command, FrameworkError, Runnable, Shutdown, config::Override};
use anyhow::{Result, anyhow};
use conflate::Merge;
use log::info;
use serde::{Deserialize, Serialize};

#[derive(Clone, Command, Default, Debug, clap::Parser, Serialize, Deserialize, Merge)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct ServeCmd {
    /// Address to bind the HTTP API server to. [default: "localhost:8080"]
    #[clap(long, value_name = "ADDRESS")]
    #[merge(strategy=conflate::option::overwrite_none)]
    address: Option<String>,
}

impl Override<RusticConfig> for ServeCmd {
    fn override_config(&self, mut config: RusticConfig) -> Result<RusticConfig, FrameworkError> {
        let mut self_config = self.clone();
        self_config.merge(config.serve);
        config.serve = self_config;
        Ok(config)
    }
}

impl Runnable for ServeCmd {
    fn run(&self) {
        if let Err(err) = self.inner_run() {
            status_err!("{}", err);
            RUSTIC_APP.shutdown(Shutdown::Crash);
        }
    }
}

impl ServeCmd {
    fn inner_run(&self) -> Result<()> {
        let config = RUSTIC_APP.config();

        let addr = config
            .serve
            .address
            .clone()
            .unwrap_or_else(|| "localhost:8080".to_string())
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| anyhow!("no address given"))?;

        info!("serving HTTP API on {addr}");

        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(async {
                let state = api::ApiState::default();
                api::serve(addr, state).await
            })?;

        Ok(())
    }
}
