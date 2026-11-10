use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::adapters::inbound::http::error_response::ApiError;
use crate::domain::server::{Server, ServerId, ServerStatus, SshCredentials};
use crate::ports::inbound::server_admin::{RegisterServerCommand, ServerAdminPort};
use crate::ports::outbound::server_executor::CommandResult;

#[derive(Clone)]
pub struct AppState {
    pub server_service: Arc<dyn ServerAdminPort>,
    pub orchestrator_service: Arc<dyn crate::ports::inbound::repo_orchestrator::RepoOrchestratorPort>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterServerRequest {
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub credentials: SshCredentials,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct MaintenanceRequest {
    pub enable: bool,
}

#[derive(Debug, Deserialize)]
pub struct ExecuteCommandRequest {
    pub command: String,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: ServerStatus,
}

pub async fn register_server(
    State(state): State<AppState>,
    Json(payload): Json<RegisterServerRequest>,
) -> Result<(StatusCode, Json<Server>), ApiError> {
    let cmd = RegisterServerCommand {
        name: payload.name,
        ip: payload.ip,
        port: payload.port,
        credentials: payload.credentials,
        tags: payload.tags.unwrap_or_default(),
    };

    let server = state.server_service.register_server(cmd).await?;
    Ok((StatusCode::CREATED, Json(server)))
}

pub async fn list_servers(
    State(state): State<AppState>,
) -> Result<Json<Vec<Server>>, ApiError> {
    let servers = state.server_service.list_servers().await?;
    Ok(Json(servers))
}

pub async fn get_server(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Server>, ApiError> {
    let server = state
        .server_service
        .get_server(&ServerId::from_uuid(id))
        .await?;
    Ok(Json(server))
}

pub async fn health_check_server(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<HealthResponse>, ApiError> {
    let status = state
        .server_service
        .health_check(&ServerId::from_uuid(id))
        .await?;
    Ok(Json(HealthResponse { status }))
}

pub async fn set_server_maintenance(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<MaintenanceRequest>,
) -> Result<Json<Server>, ApiError> {
    let server = state
        .server_service
        .set_maintenance(&ServerId::from_uuid(id), payload.enable)
        .await?;
    Ok(Json(server))
}

pub async fn execute_on_server(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ExecuteCommandRequest>,
) -> Result<Json<CommandResult>, ApiError> {
    let result = state
        .server_service
        .execute_on_server(&ServerId::from_uuid(id), &payload.command)
        .await?;
    Ok(Json(result))
}
