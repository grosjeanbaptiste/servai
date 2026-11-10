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
    catalog::{AdminCategory, CatalogItem},
    orchestration::OrchestrationJob,
    repository::ManagedRepository,
    server::ServerId,
};

#[derive(Debug, Deserialize)]
pub struct CatalogFilterQuery {
    pub category: Option<AdminCategory>,
}

#[derive(Debug, Deserialize)]
pub struct ImportCatalogRequest {
    pub custom_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApplyRecipeRequest {
    pub parameters: Option<HashMap<String, String>>,
}

pub async fn list_catalog(
    State(state): State<AppState>,
    Query(query): Query<CatalogFilterQuery>,
) -> Result<Json<Vec<CatalogItem>>, ApiError> {
    let items = state
        .orchestrator_service
        .list_catalog(query.category)
        .await?;
    Ok(Json(items))
}

pub async fn import_catalog_item(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<ImportCatalogRequest>,
) -> Result<(StatusCode, Json<ManagedRepository>), ApiError> {
    let repo = state
        .orchestrator_service
        .import_from_catalog(&id, payload.custom_name)
        .await?;
    Ok((StatusCode::CREATED, Json(repo)))
}

pub async fn apply_recipe(
    State(state): State<AppState>,
    Path((server_id, recipe_id)): Path<(Uuid, String)>,
    Json(payload): Json<ApplyRecipeRequest>,
) -> Result<(StatusCode, Json<OrchestrationJob>), ApiError> {
    let job = state
        .orchestrator_service
        .apply_recipe(
            &ServerId::from_uuid(server_id),
            &recipe_id,
            payload.parameters.unwrap_or_default(),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(job)))
}
