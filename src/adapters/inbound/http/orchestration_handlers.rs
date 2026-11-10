use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::adapters::inbound::http::{error_response::ApiError, server_handlers::AppState};
use crate::domain::{
    orchestration::{JobId, OrchestrationJob},
    repository::RepoId,
    server::ServerId,
};

#[derive(Debug, Deserialize)]
pub struct TriggerOrchestrationRequest {
    pub server_id: Uuid,
    pub repo_id: Uuid,
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
