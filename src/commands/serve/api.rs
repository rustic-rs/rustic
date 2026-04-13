//! HTTP API skeleton for `serve` command.
//!
//! This module provides a utoipa-documented endpoint to trigger backups via HTTP.
//! The endpoint accepts a TOML profile payload so all options supported by file-based
//! configuration are also supported through the API.

use std::{
	env,
	fs,
	net::SocketAddr,
	path::{Path, PathBuf},
	process::{Command, Stdio},
	time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use axum::{
	Json, Router,
	extract::State,
	http::StatusCode,
	routing::{get, post},
};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

/// Shared state for API handlers.
#[derive(Clone, Debug)]
pub struct ApiState {
	pub jobs_root: PathBuf,
}

impl Default for ApiState {
	fn default() -> Self {
		Self {
			jobs_root: env::temp_dir().join("rustic-api-jobs"),
		}
	}
}

/// API response returned when a backup job has been accepted.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub struct BackupStartResponse {
	/// Unique identifier for the spawned backup job.
	pub job_id: String,

	/// Process identifier of the spawned rustic process.
	pub pid: u32,

	/// Effective command started by this API endpoint.
	pub command: Vec<String>,

	/// Working directory used for command execution.
	pub working_directory: String,

	/// Profile file path generated for this execution.
	pub profile_file: String,
}

/// API error payload.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub struct ApiErrorResponse {
	pub message: String,
}

/// Request payload for creating a backup job.
///
/// `config_toml` is written to a temporary profile file and executed with
/// `rustic --use-profile <generated-file.toml> backup`, so every TOML option
/// supported by rustic config files can be used unchanged.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub struct BackupStartRequest {
	/// Complete rustic profile as TOML text.
	///
	/// Example:
	///
	/// [repository]
	/// repository = "/backups/repo"
	/// password = "secret"
	///
	/// [backup]
	/// no-scan = true
	/// [[backup.snapshots]]
	/// sources = ["/home/user/data"]
	pub config_toml: String,

	/// Optional profile filename to use inside the temporary jobs directory.
	/// If omitted, defaults to `api-profile.toml`.
	pub profile_filename: Option<String>,

	/// Optional extra CLI args appended after `backup`.
	/// This can be useful for quick flags like `--dry-run`.
	#[serde(default)]
	pub extra_cli_args: Vec<String>,

	/// Optional working directory for the spawned process.
	/// Defaults to the per-job temp directory.
	pub working_directory: Option<String>,
}

#[derive(OpenApi)]
#[openapi(
	paths(health, start_backup),
	components(schemas(BackupStartRequest, BackupStartResponse, ApiErrorResponse)),
	tags(
		(name = "rustic-api", description = "Rustic HTTP API skeleton")
	)
)]
pub struct ApiDoc;

/// Build the HTTP router for the serve API.
pub fn router(state: ApiState) -> Router {
	Router::new()
		.route("/health", get(health))
		.route("/v1/backup", post(start_backup))
		.with_state(state)
}

/// Start serving the HTTP API skeleton.
pub async fn serve(addr: SocketAddr, state: ApiState) -> Result<()> {
	let app = router(state);
	let listener = tokio::net::TcpListener::bind(addr)
		.await
		.with_context(|| format!("binding API socket on {addr}"))?;
	axum::serve(listener, app)
		.await
		.context("serving HTTP API")?;
	Ok(())
}

#[utoipa::path(
	get,
	path = "/health",
	tag = "rustic-api",
	responses(
		(status = 200, description = "API is running", body = String)
	)
)]
async fn health() -> &'static str {
	"ok"
}

#[utoipa::path(
	post,
	path = "/v1/backup",
	tag = "rustic-api",
	request_body = BackupStartRequest,
	responses(
		(status = 202, description = "Backup job accepted", body = BackupStartResponse),
		(status = 400, description = "Invalid request", body = ApiErrorResponse),
		(status = 500, description = "Internal error", body = ApiErrorResponse)
	)
)]
async fn start_backup(
	State(state): State<ApiState>,
	Json(req): Json<BackupStartRequest>,
) -> Result<(StatusCode, Json<BackupStartResponse>), (StatusCode, Json<ApiErrorResponse>)> {

    let config = RUSTIC_APP.config();
    if let Err(err) = config.backup.validate() {
        status_err!("{}", err);
        RUSTIC_APP.shutdown(Shutdown::Crash);
    }

    if let Err(err) = config.repository.run(|repo| self.inner_run(repo)) {
        status_err!("{}", err);
        RUSTIC_APP.shutdown(Shutdown::Crash);
    };
}

fn api_error(status: StatusCode, message: &str) -> (StatusCode, Json<ApiErrorResponse>) {
	(
		status,
		Json(ApiErrorResponse {
			message: message.to_string(),
		}),
	)
}

fn new_job_id() -> String {
	let nanos = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map_or(0, |d| d.as_nanos());
	format!("job-{nanos}")
}
