use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::collections::HashMap;
use uuid::Uuid;

use crate::adapters::inbound::http::{error_response::ApiError, server_handlers::AppState};
use crate::domain::{
    orchestration::OrchestrationJob,
    repository::RepoId,
    server::ServerId,
    stack::{MultiRepoStack, StackId, StackRepoEntry},
};

#[derive(Debug, Deserialize)]
pub struct StackRepoEntryRequest {
    pub repo_id: Uuid,
    pub order: u32,
    pub step_name: String,
    pub command: String,
    pub rollback_command: Option<String>,
    pub overrides: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateStackRequest {
    pub name: String,
    pub description: String,
    pub entries: Vec<StackRepoEntryRequest>,
}

pub async fn create_stack(
    State(state): State<AppState>,
    Json(payload): Json<CreateStackRequest>,
) -> Result<(StatusCode, Json<MultiRepoStack>), ApiError> {
    let entries = payload
        .entries
        .into_iter()
        .map(|e| StackRepoEntry {
            repo_id: RepoId::from_uuid(e.repo_id),
            order: e.order,
            step_name: e.step_name,
            command: e.command,
            rollback_command: e.rollback_command,
            overrides: e.overrides.unwrap_or_default(),
        })
        .collect();

    let stack = state
        .orchestrator_service
        .create_stack(payload.name, payload.description, entries)
        .await?;
    Ok((StatusCode::CREATED, Json(stack)))
}

pub async fn list_stacks(
    State(state): State<AppState>,
) -> Result<Json<Vec<MultiRepoStack>>, ApiError> {
    let stacks = state.orchestrator_service.list_stacks().await?;
    Ok(Json(stacks))
}

pub async fn execute_stack_handler(
    State(state): State<AppState>,
    Path((server_id, stack_id)): Path<(Uuid, Uuid)>,
) -> Result<(StatusCode, Json<OrchestrationJob>), ApiError> {
    let job = state
        .orchestrator_service
        .execute_stack(
            &ServerId::from_uuid(server_id),
            &StackId::from_uuid(stack_id),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(job)))
}
