use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::collections::HashMap;
use uuid::Uuid;

use crate::adapters::inbound::http::{error_response::ApiError, server_handlers::AppState};
use crate::domain::{
    catalog::AdminCategory,
    orchestration::OrchestrationJob,
    registry::{AdminPackage, RegistryQuery, RegistrySource, ServerProfilePreset},
    server::ServerId,
};

#[derive(Debug, Deserialize)]
pub struct RegistrySearchQuery {
    pub keyword: Option<String>,
    pub category: Option<AdminCategory>,
    pub source: Option<RegistrySource>,
    pub min_stars: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct InstallPackageRequest {
    pub parameters: Option<HashMap<String, String>>,
}

pub async fn search_registry_handler(
    State(state): State<AppState>,
    Query(query): Query<RegistrySearchQuery>,
) -> Result<Json<Vec<AdminPackage>>, ApiError> {
    let reg_query = RegistryQuery {
        keyword: query.keyword,
        category: query.category,
        source: query.source,
        min_stars: query.min_stars,
    };

    let packages = state.orchestrator_service.search_registry(reg_query).await?;
    Ok(Json(packages))
}

pub async fn list_presets_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<ServerProfilePreset>>, ApiError> {
    let presets = state.orchestrator_service.list_profile_presets().await?;
    Ok(Json(presets))
}

pub async fn install_package_handler(
    State(state): State<AppState>,
    Path((server_id, package_id)): Path<(Uuid, String)>,
    Json(payload): Json<InstallPackageRequest>,
) -> Result<(StatusCode, Json<OrchestrationJob>), ApiError> {
    let job = state
        .orchestrator_service
        .install_package_with_dependencies(
            &ServerId::from_uuid(server_id),
            &package_id,
            payload.parameters.unwrap_or_default(),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(job)))
}

pub async fn apply_preset_handler(
    State(state): State<AppState>,
    Path((server_id, preset_id)): Path<(Uuid, String)>,
) -> Result<(StatusCode, Json<OrchestrationJob>), ApiError> {
    let job = state
        .orchestrator_service
        .apply_profile_preset(&ServerId::from_uuid(server_id), &preset_id)
        .await?;
    Ok((StatusCode::CREATED, Json(job)))
}
