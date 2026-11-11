use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

use servai::{
    adapters::{
        inbound::http::create_router,
        outbound::{
            memory::{
                job_store::InMemoryJobStore, repo_store::InMemoryRepoStore,
                server_store::InMemoryServerStore,
            },
            mock::{MockGitProvider, MockServerExecutor},
        },
    },
    application::{
        orchestration_service::OrchestrationService,
        server_service::ServerService,
    },
    ports::{
        inbound::{
            repo_orchestrator::RepoOrchestratorPort,
            server_admin::ServerAdminPort,
        },
    },
};

fn setup_test_app() -> axum::Router {
    let server_store = Arc::new(InMemoryServerStore::new());
    let repo_store = Arc::new(InMemoryRepoStore::new());
    let job_store = Arc::new(InMemoryJobStore::new());
    let executor = Arc::new(MockServerExecutor::default());
    let git_provider = Arc::new(MockGitProvider::default());

    let server_service: Arc<dyn ServerAdminPort> =
        Arc::new(ServerService::new(server_store.clone(), executor.clone()));
    let orchestrator_service: Arc<dyn RepoOrchestratorPort> = Arc::new(
        OrchestrationService::new(server_store, repo_store, job_store, executor, git_provider),
    );

    create_router(server_service, orchestrator_service)
}

#[tokio::test]
async fn test_api_healthcheck() {
    let app = setup_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body_json["status"], "ok");
}

#[tokio::test]
async fn test_bioxus_api_server_full_lifecycle() {
    let app = setup_test_app();

    // 1. Register Server via POST /api/v1/servers
    let create_payload = json!({
        "name": "srv-bioxus-01",
        "ip": "192.168.1.50",
        "port": 22,
        "credentials": {
            "Agent": null
        },
        "tags": ["web", "production"]
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/servers")
                .header("content-type", "application/json")
                .body(Body::from(create_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::CREATED);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let created: Value = serde_json::from_slice(&body).unwrap();
    let server_id = created["id"].as_str().expect("valid server id");
    assert_eq!(created["status"], "Pending");

    // 2. GET /api/v1/servers should list the server
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/servers")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let list: Vec<Value> = serde_json::from_slice(
        &res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["id"], server_id);

    // 3. Healthcheck ping POST /api/v1/servers/{id}/health
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/servers/{}/health", server_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let health_resp: Value = serde_json::from_slice(
        &res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(health_resp["status"], "Online");

    // 4. Execute administrative command POST /api/v1/servers/{id}/execute
    let cmd_payload = json!({
        "command": "uptime"
    });
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/servers/{}/execute", server_id))
                .header("content-type", "application/json")
                .body(Body::from(cmd_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let exec_res: Value = serde_json::from_slice(
        &res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(exec_res["exit_code"], 0);

    // 5. Toggle Maintenance POST /api/v1/servers/{id}/maintenance
    let maint_payload = json!({
        "enable": true
    });
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/servers/{}/maintenance", server_id))
                .header("content-type", "application/json")
                .body(Body::from(maint_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let maint_res: Value = serde_json::from_slice(
        &res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(maint_res["status"], "Maintenance");
}

#[tokio::test]
async fn test_bioxus_api_orchestration_flow() {
    let app = setup_test_app();

    // 1. Register and healthcheck server
    let srv_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/servers")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "target-srv",
                        "ip": "10.0.0.1",
                        "port": 22,
                        "credentials": { "Agent": null },
                        "tags": []
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let srv_json: Value = serde_json::from_slice(
        &srv_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    let server_id = srv_json["id"].as_str().unwrap();

    // Activate server
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/servers/{}/health", server_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // 2. Register repository POST /api/v1/repos
    let repo_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "ansible-common",
                        "url": "https://github.com/myorg/ansible-common.git",
                        "target_ref": { "Branch": "main" },
                        "kind": "Playbook"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(repo_res.status(), StatusCode::CREATED);
    let repo_json: Value = serde_json::from_slice(
        &repo_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    let repo_id = repo_json["id"].as_str().unwrap();

    // 3. Sync repository POST /api/v1/repos/{id}/sync
    let sync_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/repos/{}/sync", repo_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(sync_res.status(), StatusCode::OK);

    // 4. Trigger orchestration POST /api/v1/orchestrations
    let orch_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/orchestrations")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "server_id": server_id,
                        "repo_id": repo_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(orch_res.status(), StatusCode::CREATED);
    let orch_json: Value = serde_json::from_slice(
        &orch_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(orch_json["status"], "Succeeded");

    let job_id = orch_json["id"].as_str().unwrap();

    // 5. Inspect job GET /api/v1/orchestrations/{id}
    let get_job_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/orchestrations/{}", job_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_job_res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_bioxus_api_validation_and_not_found() {
    let app = setup_test_app();

    // Invalid server payload
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/servers")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "", // Invalid empty name
                        "ip": "invalid host!",
                        "port": 0,
                        "credentials": { "Agent": null },
                        "tags": []
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // Server not found
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/servers/00000000-0000-0000-0000-000000000000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_bioxus_api_metrics_and_service_management() {
    let app = setup_test_app();

    // Register and activate server
    let srv_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/servers")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "bioxus-metrics-node",
                        "ip": "10.0.0.10",
                        "port": 22,
                        "credentials": { "Agent": null },
                        "tags": []
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let srv_json: Value = serde_json::from_slice(
        &srv_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    let server_id = srv_json["id"].as_str().unwrap();

    // Activate
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/servers/{}/health", server_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // 1. GET /api/v1/servers/{id}/metrics
    let metrics_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/servers/{}/metrics", server_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(metrics_res.status(), StatusCode::OK);
    let metrics_json: Value = serde_json::from_slice(
        &metrics_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert!(metrics_json["cpu_usage_pct"].as_f64().is_some());
    assert!(metrics_json["ram_used_mb"].as_u64().is_some());

    // 2. POST /api/v1/servers/{id}/services/{name}/action
    let svc_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/servers/{}/services/caddy/action", server_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "action": "Restart"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(svc_res.status(), StatusCode::OK);
    let svc_json: Value = serde_json::from_slice(
        &svc_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(svc_json["status"], "Active");

    // 3. POST /api/v1/servers/{id}/reboot
    let reboot_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/servers/{}/reboot", server_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(reboot_res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_bioxus_api_drift_and_cancellation() {
    let app = setup_test_app();

    // Register and activate server
    let srv_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/servers")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "bioxus-drift-srv",
                        "ip": "10.0.0.11",
                        "port": 22,
                        "credentials": { "Agent": null },
                        "tags": []
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let srv_json: Value = serde_json::from_slice(
        &srv_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    let server_id = srv_json["id"].as_str().unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/servers/{}/health", server_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Register repo
    let repo_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "bioxus-configs",
                        "url": "https://github.com/myorg/bioxus-configs.git",
                        "target_ref": { "Branch": "main" },
                        "kind": "RawConfig"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let repo_json: Value = serde_json::from_slice(
        &repo_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    let repo_id = repo_json["id"].as_str().unwrap();

    // POST /api/v1/orchestrations/drift
    let drift_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/orchestrations/drift")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "server_id": server_id,
                        "repo_id": repo_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(drift_res.status(), StatusCode::OK);
    let drift_json: Value = serde_json::from_slice(
        &drift_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(drift_json["has_drift"], false);
}

#[tokio::test]
async fn test_bioxus_api_catalog_and_recipe_application() {
    let app = setup_test_app();

    // 1. GET /api/v1/catalog
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/catalog")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let catalog: Vec<Value> = serde_json::from_slice(
        &res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert!(catalog.len() >= 6);
    assert!(catalog.iter().any(|item| item["id"] == "linux-hardening"));

    // 2. POST /api/v1/catalog/linux-hardening/import
    let import_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/catalog/linux-hardening/import")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "custom_name": "custom-hardening"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(import_res.status(), StatusCode::CREATED);
    let imported: Value = serde_json::from_slice(
        &import_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(imported["name"], "custom-hardening");

    // 3. Register server and apply recipe directly
    let srv_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/servers")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "recipe-srv",
                        "ip": "10.0.0.77",
                        "port": 22,
                        "credentials": { "Agent": null },
                        "tags": []
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let srv_json: Value = serde_json::from_slice(
        &srv_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    let server_id = srv_json["id"].as_str().unwrap();

    // Activate server
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/servers/{}/health", server_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // POST /api/v1/servers/:id/recipes/:recipe_id/apply
    let apply_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/servers/{}/recipes/docker-engine-compose/apply",
                    server_id
                ))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "parameters": {
                            "docker_storage_driver": "overlay2"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(apply_res.status(), StatusCode::CREATED);
    let job_json: Value = serde_json::from_slice(
        &apply_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(job_json["status"], "Succeeded");
}

#[tokio::test]
async fn test_bioxus_api_multi_repo_stack_endpoints() {
    let app = setup_test_app();

    // 1. Register two repositories
    let r1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "r1-hardening",
                        "url": "https://github.com/dev-sec/ansible-collection-hardening.git",
                        "target_ref": { "Branch": "main" },
                        "kind": "Playbook"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let r1_json: Value = serde_json::from_slice(
        &r1.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    let r1_id = r1_json["id"].as_str().unwrap();

    let r2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "r2-docker",
                        "url": "https://github.com/geerlingguy/ansible-role-docker.git",
                        "target_ref": { "Branch": "master" },
                        "kind": "Playbook"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let r2_json: Value = serde_json::from_slice(
        &r2.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    let r2_id = r2_json["id"].as_str().unwrap();

    // 2. POST /api/v1/stacks (compose multi-repo stack)
    let stack_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/stacks")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "bioxus-web-stack",
                        "description": "Hardening + Docker stack",
                        "entries": [
                            {
                                "repo_id": r1_id,
                                "order": 1,
                                "step_name": "h-step",
                                "command": "apply.sh",
                                "rollback_command": null,
                                "overrides": {}
                            },
                            {
                                "repo_id": r2_id,
                                "order": 2,
                                "step_name": "d-step",
                                "command": "compose.sh",
                                "rollback_command": null,
                                "overrides": {}
                            }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(stack_res.status(), StatusCode::CREATED);
    let stack_json: Value = serde_json::from_slice(
        &stack_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    let stack_id = stack_json["id"].as_str().unwrap();

    // 3. GET /api/v1/stacks
    let list_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/stacks")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(list_res.status(), StatusCode::OK);
    let list_json: Vec<Value> = serde_json::from_slice(
        &list_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(list_json.len(), 1);
    assert_eq!(list_json[0]["id"], stack_id);

    // 4. Register server & execute stack
    let srv_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/servers")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "stack-target-node",
                        "ip": "10.0.0.100",
                        "port": 22,
                        "credentials": { "Agent": null },
                        "tags": []
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let srv_json: Value = serde_json::from_slice(
        &srv_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    let server_id = srv_json["id"].as_str().unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/servers/{}/health", server_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // POST /api/v1/servers/:id/stacks/:stack_id/apply
    let apply_stack_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/servers/{}/stacks/{}/apply",
                    server_id, stack_id
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(apply_stack_res.status(), StatusCode::CREATED);
    let job_out: Value = serde_json::from_slice(
        &apply_stack_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(job_out["status"], "Succeeded");
}

#[tokio::test]
async fn test_bioxus_api_registry_search_and_preset_installation() {
    let app = setup_test_app();

    // 1. GET /api/v1/registry/search?keyword=docker
    let search_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/registry/search?keyword=docker")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(search_res.status(), StatusCode::OK);
    let pkgs: Vec<Value> = serde_json::from_slice(
        &search_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert!(!pkgs.is_empty());
    assert!(pkgs.iter().any(|p| p["id"] == "geerlingguy-docker"));

    // 2. GET /api/v1/registry/presets
    let presets_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/registry/presets")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(presets_res.status(), StatusCode::OK);
    let presets: Vec<Value> = serde_json::from_slice(
        &presets_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert!(presets.iter().any(|p| p["id"] == "secure-web-baseline"));

    // 3. Register and activate server
    let srv_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/servers")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "registry-test-node",
                        "ip": "10.0.0.123",
                        "port": 22,
                        "credentials": { "Agent": null },
                        "tags": []
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let srv_json: Value = serde_json::from_slice(
        &srv_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    let server_id = srv_json["id"].as_str().unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/servers/{}/health", server_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // 4. POST /api/v1/servers/:id/presets/:preset_id/apply
    let apply_preset_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/servers/{}/presets/secure-web-baseline/apply",
                    server_id
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(apply_preset_res.status(), StatusCode::CREATED);
    let job_out: Value = serde_json::from_slice(
        &apply_preset_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(job_out["status"], "Succeeded");
}

#[tokio::test]
async fn test_bioxus_api_opentofu_and_ansible_engine_endpoints() {
    let app = setup_test_app();

    // 1. Register OpenTofu repo
    let repo_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/repos")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "tofu-infra-prod",
                        "url": "https://github.com/infra/prod-cluster.git",
                        "target_ref": { "Branch": "main" },
                        "kind": "OpenTofu"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(repo_res.status(), StatusCode::CREATED);
    let repo_json: Value = serde_json::from_slice(
        &repo_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    let repo_id = repo_json["id"].as_str().unwrap();

    // 2. POST /api/v1/tofu/plan
    let plan_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/tofu/plan")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "repo_id": repo_id,
                        "vars": { "instance_count": "2" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(plan_res.status(), StatusCode::OK);
    let plan_json: Value = serde_json::from_slice(
        &plan_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(plan_json["summary"]["to_add"], 2);
    assert_eq!(plan_json["summary"]["to_change"], 0);
    assert_eq!(plan_json["summary"]["to_destroy"], 0);

    // 3. POST /api/v1/tofu/apply
    let apply_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/tofu/apply")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "repo_id": repo_id,
                        "vars": { "instance_count": "2" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(apply_res.status(), StatusCode::OK);
    let apply_json: Value = serde_json::from_slice(
        &apply_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(apply_json["success"], true);
    assert_eq!(apply_json["resources_created"], 2);
    assert_eq!(
        apply_json["outputs"]["cluster_endpoint"],
        "https://10.0.0.10:6443"
    );

    // 4. GET /api/v1/tofu/state/:repo_id
    let state_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/tofu/state/{}", repo_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(state_res.status(), StatusCode::OK);
    let state_json: Value = serde_json::from_slice(
        &state_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(state_json["serial"], 1);
    assert_eq!(state_json["lineage"], "mock-lineage-123");

    // 5. POST /api/v1/tofu/destroy
    let destroy_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/tofu/destroy")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "repo_id": repo_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(destroy_res.status(), StatusCode::OK);
    let destroy_json: Value = serde_json::from_slice(
        &destroy_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(destroy_json["success"], true);
    assert_eq!(destroy_json["resources_deleted"], 2);

    // 6. Register server and bring it online
    let srv_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/servers")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "bioxus-node-ansible",
                        "ip": "10.10.10.1",
                        "port": 22,
                        "credentials": "Agent",
                        "tags": ["prod", "bioxus"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let srv_json: Value = serde_json::from_slice(
        &srv_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    let server_id = srv_json["id"].as_str().unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/servers/{}/health", server_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // 7. POST /api/v1/servers/:id/ansible/playbook
    let pb_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/servers/{}/ansible/playbook", server_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "playbook": "site.yml",
                        "vars": { "env": "production" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(pb_res.status(), StatusCode::OK);
    let pb_json: Value = serde_json::from_slice(
        &pb_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(pb_json["ok_count"], 3);
    assert_eq!(pb_json["changed_count"], 1);
    assert_eq!(pb_json["failed_count"], 0);

    // 8. POST /api/v1/servers/:id/ansible/module
    let mod_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/servers/{}/ansible/module", server_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "module": "command",
                        "args": "uptime"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(mod_res.status(), StatusCode::OK);
    let mod_json: Value = serde_json::from_slice(
        &mod_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(mod_json["module"], "command");
    assert_eq!(mod_json["changed"], true);
    assert_eq!(mod_json["rc"], 0);

    // 9. GET /api/v1/servers/:id/ansible/facts
    let facts_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/servers/{}/ansible/facts", server_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(facts_res.status(), StatusCode::OK);
    let facts_json: Value = serde_json::from_slice(
        &facts_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(facts_json["distribution"], "Ubuntu");
    assert_eq!(facts_json["distribution_version"], "22.04");
    assert_eq!(facts_json["memtotal_mb"], 8192);
}




