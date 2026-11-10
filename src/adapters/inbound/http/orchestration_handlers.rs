use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::adapters::inbound::http::{error_response::ApiError, server_handlers::AppState};
use crate::domain::{
    orchestration::{JobId, OrchestrationJob},
    repository::{DriftReport, RepoId},
    server::ServerId,
};

#[derive(Debug, Deserialize)]
pub struct TriggerOrchestrationRequest {
    pub server_id: Uuid,
    pub repo_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct DriftDetectionRequest {
    pub server_id: Uuid,
    pub repo_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct DriftResponse {
    pub has_drift: bool,
    pub report: DriftReport,
}

#[derive(Debug, Deserialize)]
pub struct CancelJobRequest {
    pub reason: Option<String>,
}

pub async fn trigger_orchestration(
    State(state): State<AppState>,
    Json(payload): Json<TriggerOrchestrationRequest>,
) -> Result<(StatusCode, Json<OrchestrationJob>), ApiError> {
    let server_id = ServerId::from_uuid(payload.server_id);
    let repo_id = RepoId::from_uuid(payload.repo_id);

    let job = state
        .orchestrator_service
        .trigger_orchestration(&server_id, &repo_id)
        .await?;
    Ok((StatusCode::CREATED, Json(job)))
}

pub async fn get_orchestration(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<OrchestrationJob>, ApiError> {
    let job = state
        .orchestrator_service
        .get_job(&JobId::from_uuid(id))
        .await?;
    Ok(Json(job))
}

pub async fn detect_drift_handler(
    State(state): State<AppState>,
    Json(payload): Json<DriftDetectionRequest>,
) -> Result<Json<DriftResponse>, ApiError> {
    let server_id = ServerId::from_uuid(payload.server_id);
    let repo_id = RepoId::from_uuid(payload.repo_id);

    let report = state
        .orchestrator_service
        .detect_drift(&server_id, &repo_id)
        .await?;

    let has_drift = report.has_drift();
    Ok(Json(DriftResponse { has_drift, report }))
}

pub async fn cancel_orchestration(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<CancelJobRequest>,
) -> Result<Json<OrchestrationJob>, ApiError> {
    let reason = payload.reason.unwrap_or_else(|| "User cancelled".to_string());
    let job = state
        .orchestrator_service
        .cancel_job(&JobId::from_uuid(id), &reason)
        .await?;
    Ok(Json(job))
}
