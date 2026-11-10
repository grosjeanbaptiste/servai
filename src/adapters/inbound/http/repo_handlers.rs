use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::adapters::inbound::http::{error_response::ApiError, server_handlers::AppState};
use crate::domain::repository::{GitRef, ManagedRepository, RepoId, RepoKind};
use crate::ports::inbound::repo_orchestrator::RegisterRepoCommand;

#[derive(Debug, Deserialize)]
pub struct RegisterRepoRequest {
    pub name: String,
    pub url: String,
    pub target_ref: GitRef,
    pub kind: RepoKind,
}

pub async fn register_repo(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRepoRequest>,
) -> Result<(StatusCode, Json<ManagedRepository>), ApiError> {
    let cmd = RegisterRepoCommand {
        name: payload.name,
        url: payload.url,
        target_ref: payload.target_ref,
        kind: payload.kind,
    };

    let repo = state.orchestrator_service.register_repo(cmd).await?;
    Ok((StatusCode::CREATED, Json(repo)))
}

pub async fn list_repos(
    State(state): State<AppState>,
) -> Result<Json<Vec<ManagedRepository>>, ApiError> {
    let repos = state.orchestrator_service.list_repos().await?;
    Ok(Json(repos))
}

pub async fn get_repo(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ManagedRepository>, ApiError> {
    let repo = state
        .orchestrator_service
        .get_repo(&RepoId::from_uuid(id))
        .await?;
    Ok(Json(repo))
}

pub async fn sync_repo(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ManagedRepository>, ApiError> {
    let repo = state
        .orchestrator_service
        .sync_repo(&RepoId::from_uuid(id))
        .await?;
    Ok(Json(repo))
}
