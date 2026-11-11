use axum::{
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::adapters::inbound::http::{
    catalog_handlers::{apply_recipe, import_catalog_item, list_catalog},
    engine_handlers::{
        add_wireguard_peer_handler, apply_crossplane_claim_handler, apply_tofu_handler,
        configure_wireguard_handler, delete_crossplane_claim_handler, destroy_tofu_handler,
        gather_ansible_facts_handler, generate_wireguard_keypair_handler,
        get_crossplane_claim_handler, get_tofu_state_handler, get_wireguard_status_handler,
        list_crossplane_providers_handler, list_crossplane_resources_handler,
        list_wireguard_peers_handler, plan_tofu_handler, remove_wireguard_peer_handler,
        run_ansible_module_handler, run_ansible_playbook_handler,
    },
    orchestration_handlers::{
        cancel_orchestration, detect_drift_handler, get_orchestration, trigger_orchestration,
    },
    registry_handlers::{
        apply_preset_handler, install_package_handler, list_presets_handler, search_registry_handler,
    },
    repo_handlers::{get_repo, list_repos, register_repo, sync_repo},
    server_handlers::{
        execute_on_server, get_server, get_server_metrics, health_check_server, list_servers,
        manage_service, reboot_server, register_server, set_server_maintenance, AppState,
    },
    stack_handlers::{create_stack, execute_stack_handler, list_stacks},
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
        // Server administration routes (for Bioxus UI)
        .route("/api/v1/servers", post(register_server).get(list_servers))
        .route("/api/v1/servers/{id}", get(get_server))
        .route("/api/v1/servers/{id}/health", post(health_check_server))
        .route("/api/v1/servers/{id}/maintenance", post(set_server_maintenance))
        .route("/api/v1/servers/{id}/execute", post(execute_on_server))
        .route("/api/v1/servers/{id}/metrics", get(get_server_metrics))
        .route(
            "/api/v1/servers/{id}/services/{service}/action",
            post(manage_service),
        )
        .route("/api/v1/servers/{id}/reboot", post(reboot_server))
        .route(
            "/api/v1/servers/{id}/recipes/{recipe_id}/apply",
            post(apply_recipe),
        )
        .route(
            "/api/v1/servers/{id}/stacks/{stack_id}/apply",
            post(execute_stack_handler),
        )
        .route(
            "/api/v1/servers/{id}/packages/{package_id}/install",
            post(install_package_handler),
        )
        .route(
            "/api/v1/servers/{id}/presets/{preset_id}/apply",
            post(apply_preset_handler),
        )
        // Repository management routes
        .route("/api/v1/repos", post(register_repo).get(list_repos))
        .route("/api/v1/repos/{id}", get(get_repo))
        .route("/api/v1/repos/{id}/sync", post(sync_repo))
        // Catalog of curated server administration repos
        .route("/api/v1/catalog", get(list_catalog))
        .route("/api/v1/catalog/{id}/import", post(import_catalog_item))
        // Multi-source server administration registry
        .route("/api/v1/registry/search", get(search_registry_handler))
        .route("/api/v1/registry/presets", get(list_presets_handler))
        // Multi-repo stacks
        .route("/api/v1/stacks", post(create_stack).get(list_stacks))
        // Orchestration routes
        .route("/api/v1/orchestrations", post(trigger_orchestration))
        .route("/api/v1/orchestrations/{id}", get(get_orchestration))
        .route("/api/v1/orchestrations/{id}/cancel", post(cancel_orchestration))
        .route("/api/v1/orchestrations/drift", post(detect_drift_handler))
        // OpenTofu orchestration routes (for Bioxus)
        .route("/api/v1/tofu/plan", post(plan_tofu_handler))
        .route("/api/v1/tofu/apply", post(apply_tofu_handler))
        .route("/api/v1/tofu/destroy", post(destroy_tofu_handler))
        .route("/api/v1/tofu/state/{repo_id}", get(get_tofu_state_handler))
        // Ansible execution routes (for Bioxus)
        .route(
            "/api/v1/servers/{id}/ansible/playbook",
            post(run_ansible_playbook_handler),
        )
        .route(
            "/api/v1/servers/{id}/ansible/module",
            post(run_ansible_module_handler),
        )
        .route(
            "/api/v1/servers/{id}/ansible/facts",
            get(gather_ansible_facts_handler),
        )
        // Crossplane Control Plane orchestration routes (for Bioxus)
        .route("/api/v1/crossplane/claims", post(apply_crossplane_claim_handler))
        .route(
            "/api/v1/crossplane/claims/{namespace}/{name}",
            get(get_crossplane_claim_handler).delete(delete_crossplane_claim_handler),
        )
        .route(
            "/api/v1/crossplane/resources",
            get(list_crossplane_resources_handler),
        )
        .route(
            "/api/v1/crossplane/providers",
            get(list_crossplane_providers_handler),
        )
        // WireGuard VPN administration routes (for Bioxus)
        .route(
            "/api/v1/servers/{id}/wireguard",
            post(configure_wireguard_handler),
        )
        .route(
            "/api/v1/servers/{id}/wireguard/{iface}/status",
            get(get_wireguard_status_handler),
        )
        .route(
            "/api/v1/servers/{id}/wireguard/{iface}/peers",
            get(list_wireguard_peers_handler).post(add_wireguard_peer_handler),
        )
        .route(
            "/api/v1/servers/{id}/wireguard/{iface}/peers/{pubkey}",
            axum::routing::delete(remove_wireguard_peer_handler),
        )
        .route(
            "/api/v1/wireguard/keypair",
            post(generate_wireguard_keypair_handler),
        )
        .with_state(state)
}
