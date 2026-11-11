use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use std::collections::HashMap;
use uuid::Uuid;

use crate::adapters::inbound::http::{error_response::ApiError, server_handlers::AppState};
use crate::domain::{
    engines::{
        ansible::{AnsibleFacts, AnsibleModuleResult, AnsibleRunResult},
        opentofu::{TofuApplyResult, TofuPlanResult, TofuState},
    },
    repository::RepoId,
    server::ServerId,
};

#[derive(Debug, Deserialize)]
pub struct TofuPlanRequest {
    pub repo_id: Uuid,
    pub vars: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct TofuApplyRequest {
    pub repo_id: Uuid,
    pub vars: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct AnsiblePlaybookRequest {
    pub playbook: String,
    pub vars: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct AnsibleModuleRequest {
    pub module: String,
    pub args: String,
}

pub async fn plan_tofu_handler(
    State(state): State<AppState>,
    Json(payload): Json<TofuPlanRequest>,
) -> Result<Json<TofuPlanResult>, ApiError> {
    let repo_id = RepoId::from_uuid(payload.repo_id);
    let result = state
        .orchestrator_service
        .plan_tofu(&repo_id, payload.vars.unwrap_or_default())
        .await?;
    Ok(Json(result))
}

pub async fn apply_tofu_handler(
    State(state): State<AppState>,
    Json(payload): Json<TofuApplyRequest>,
) -> Result<Json<TofuApplyResult>, ApiError> {
    let repo_id = RepoId::from_uuid(payload.repo_id);
    let result = state
        .orchestrator_service
        .apply_tofu(&repo_id, payload.vars.unwrap_or_default())
        .await?;
    Ok(Json(result))
}

pub async fn destroy_tofu_handler(
    State(state): State<AppState>,
    Json(payload): Json<TofuApplyRequest>,
) -> Result<Json<TofuApplyResult>, ApiError> {
    let repo_id = RepoId::from_uuid(payload.repo_id);
    let result = state
        .orchestrator_service
        .destroy_tofu(&repo_id, payload.vars.unwrap_or_default())
        .await?;
    Ok(Json(result))
}

pub async fn get_tofu_state_handler(
    State(state): State<AppState>,
    Path(repo_id): Path<Uuid>,
) -> Result<Json<TofuState>, ApiError> {
    let id = RepoId::from_uuid(repo_id);
    let state_res = state.orchestrator_service.get_tofu_state(&id).await?;
    Ok(Json(state_res))
}

pub async fn run_ansible_playbook_handler(
    State(state): State<AppState>,
    Path(server_id): Path<Uuid>,
    Json(payload): Json<AnsiblePlaybookRequest>,
) -> Result<Json<AnsibleRunResult>, ApiError> {
    let id = ServerId::from_uuid(server_id);
    let result = state
        .orchestrator_service
        .run_ansible_playbook(&id, &payload.playbook, payload.vars.unwrap_or_default())
        .await?;
    Ok(Json(result))
}

pub async fn run_ansible_module_handler(
    State(state): State<AppState>,
    Path(server_id): Path<Uuid>,
    Json(payload): Json<AnsibleModuleRequest>,
) -> Result<Json<AnsibleModuleResult>, ApiError> {
    let id = ServerId::from_uuid(server_id);
    let result = state
        .server_service
        .run_ansible_module(&id, &payload.module, &payload.args)
        .await?;
    Ok(Json(result))
}

pub async fn gather_ansible_facts_handler(
    State(state): State<AppState>,
    Path(server_id): Path<Uuid>,
) -> Result<Json<AnsibleFacts>, ApiError> {
    let id = ServerId::from_uuid(server_id);
    let facts = state.server_service.gather_ansible_facts(&id).await?;
    Ok(Json(facts))
}
