use axum::{
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::adapters::inbound::http::{
    orchestration_handlers::{get_orchestration, trigger_orchestration},
    repo_handlers::{get_repo, list_repos, register_repo, sync_repo},
    server_handlers::{
        execute_on_server, get_server, health_check_server, list_servers, register_server,
        set_server_maintenance, AppState,
    },
};
use crate::ports::inbound::{
    repo_orchestrator::RepoOrchestratorPort,
    server_admin::ServerAdminPort,
};

async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "servai",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

pub fn create_router(
    server_service: Arc<dyn ServerAdminPort>,
    orchestrator_service: Arc<dyn RepoOrchestratorPort>,
) -> Router {
    let state = AppState {
        server_service,
        orchestrator_service,
    };

    Router::new()
        .route("/api/v1/health", get(health_check))
        // Server administration routes (for bioxus UI)
        .route("/api/v1/servers", post(register_server).get(list_servers))
        .route("/api/v1/servers/{id}", get(get_server))
        .route("/api/v1/servers/{id}/health", post(health_check_server))
        .route("/api/v1/servers/{id}/maintenance", post(set_server_maintenance))
        .route("/api/v1/servers/{id}/execute", post(execute_on_server))
        // Repository management routes
        .route("/api/v1/repos", post(register_repo).get(list_repos))
        .route("/api/v1/repos/{id}", get(get_repo))
        .route("/api/v1/repos/{id}/sync", post(sync_repo))
        // Orchestration routes
        .route("/api/v1/orchestrations", post(trigger_orchestration))
        .route("/api/v1/orchestrations/{id}", get(get_orchestration))
        .with_state(state)
}
