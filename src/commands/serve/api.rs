//! HTTP API skeleton for `serve` command.
//!
//! This module provides a utoipa-documented endpoint to trigger backups via HTTP.
//! The endpoint accepts a TOML profile payload so all options supported by file-based
//! configuration are also supported through the API.

use std::{
	collections::HashMap,
	fs,
	net::SocketAddr,
	sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use axum::{
	Json, Router,
	extract::{Path as AxumPath, State},
	http::StatusCode,
	routing::{get, post},
};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};
use uuid::Uuid;

/// Status of a backup job.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum JobStatus {
	/// The job is currently running.
	Running,
	/// The job completed successfully.
	Completed,
	/// The job terminated with an error.
	Failed,
}

/// Shared state for API handlers.
#[derive(Clone, Debug)]
pub struct ApiState {
	/// In-memory map of job_id -> status for all submitted backup jobs.
	pub jobs: Arc<Mutex<HashMap<String, JobStatus>>>,
}

impl Default for ApiState {
	fn default() -> Self {
		Self {
			jobs: Arc::new(Mutex::new(HashMap::new())),
		}
	}
}

/// API response returned when a backup job has been accepted.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub struct BackupStartResponse {
	/// Unique identifier for the backup job.
    /// Please note that at this time rustic can only run one backup job at a time, 
    /// so that there will be at most one active job_id. 
    /// This parameter allows the API to be extended in the future to support multiple concurrent jobs if needed.
	pub job_id: String,
}

/// API error payload.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub struct ApiErrorResponse {
	pub message: String,
}

/// Request payload for creating a backup job.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub struct BackupStartRequest {
	/// Optional profile name to define which sources to backup
    /// Equivalent to the --name CLI option of "backup" command.
	pub profile_name: Option<String>,
}

/// Response body for GET /backup/{job_id}.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub struct BackupJobStatusResponse {
	/// The job identifier.
	pub job_id: String,
	/// Current status of the job.
	pub status: JobStatus,
}

#[derive(OpenApi)]
#[openapi(
	paths(health, start_backup, get_backup_status),
	components(schemas(BackupStartRequest, BackupStartResponse, BackupJobStatusResponse, JobStatus, ApiErrorResponse)),
	tags(
		(name = "rustic-api", description = "Rustic HTTP API skeleton")
	)
)]
pub struct ApiDoc;

/// Write the generated OpenAPI schema to the given file path.
pub fn write_openapi_schema(path: &std::path::Path) -> Result<()> {
	let schema = ApiDoc::openapi();
	let json = serde_json::to_string_pretty(&schema).context("serializing OpenAPI schema")?;

	if let Some(parent) = path.parent() {
		fs::create_dir_all(parent)
			.with_context(|| format!("creating schema output directory {}", parent.display()))?;
	}

	fs::write(path, json)
		.with_context(|| format!("writing OpenAPI schema to {}", path.display()))?;

	Ok(())
}

/// Build the HTTP router for the serve API.
pub fn router(state: ApiState) -> Router {
	Router::new()
		.route("/health", get(health))
		.route("/backup", post(start_backup))
		.route("/backup/{job_id}", get(get_backup_status))
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


/// Respond to /health endpoint, used for health checks and testing connectivity to the API.
/// 
/// Development note: test this with:
/// 
/// curl -i -X GET http://localhost:8080/health
///
/// # Returns
///
/// Returns "ok" if the API is running.
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

/// Respond to /backup endpoint, used for starting backup jobs.
/// 
/// Development note: test this with:
/// 
/// curl -i -X POST http://localhost:8080/backup -H "Content-Type: application/json" -d @tests/http-server/backup-request.json
///
/// # Returns
///
/// Returns BackupStartResponse in case of success
#[utoipa::path(
	post,
	path = "/backup",
	tag = "rustic-api",
	request_body = BackupStartRequest,
	responses(
		(status = 202, description = "Backup job accepted", body = BackupStartResponse),
		(status = 400, description = "Invalid request", body = ApiErrorResponse),
		(status = 409, description = "A backup job is already running", body = ApiErrorResponse),
		(status = 500, description = "Internal error", body = ApiErrorResponse)
	)
)]
async fn start_backup(
	State(state): State<ApiState>,
	Json(req): Json<BackupStartRequest>,
) -> Result<(StatusCode, Json<BackupStartResponse>), (StatusCode, Json<ApiErrorResponse>)> {
    // TODO: kick off a new backup job here.
    // Note that we just need to start the backup job asynchronously and return
    // a job_id immediately, without waiting for the backup to complete.
    let _ = req;

    let mut jobs = state
        .jobs
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    // Enforce single-job constraint: reject if any job is still Running.
    if let Some(active_id) = jobs
        .iter()
        .find_map(|(id, s)| (*s == JobStatus::Running).then(|| id.clone()))
    {
        return Err(api_error(
            StatusCode::CONFLICT,
            &format!("backup job '{active_id}' is already running"),
        ));
    }

    let job_id = Uuid::new_v4().to_string();
    let _ = jobs.insert(job_id.clone(), JobStatus::Running);

	Ok((
		StatusCode::ACCEPTED,
		Json(BackupStartResponse { job_id }),
	))
}

/// Get the status of a backup job.
///
/// Development note: test this with:
///
/// curl -i -X GET http://localhost:8080/backup/<job_id>
#[utoipa::path(
	get,
	path = "/backup/{job_id}",
	tag = "rustic-api",
	params(
		("job_id" = String, Path, description = "Job identifier returned by POST /backup")
	),
	responses(
		(status = 200, description = "Job status", body = BackupJobStatusResponse),
		(status = 404, description = "Job not found", body = ApiErrorResponse)
	)
)]
async fn get_backup_status(
	State(state): State<ApiState>,
	AxumPath(job_id): AxumPath<String>,
) -> Result<Json<BackupJobStatusResponse>, (StatusCode, Json<ApiErrorResponse>)> {
	let jobs = state
		.jobs
		.lock()
		.unwrap_or_else(|e| e.into_inner());
	match jobs.get(&job_id) {
		Some(status) => Ok(Json(BackupJobStatusResponse {
			job_id,
			status: status.clone(),
		})),
		None => Err(api_error(
			StatusCode::NOT_FOUND,
			&format!("job '{job_id}' not found"),
		)),
	}
}

fn api_error(status: StatusCode, message: &str) -> (StatusCode, Json<ApiErrorResponse>) {
	(
		status,
		Json(ApiErrorResponse {
			message: message.to_string(),
		}),
	)
}
