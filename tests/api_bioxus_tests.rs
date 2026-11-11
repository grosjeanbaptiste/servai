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

#[tokio::test]
async fn test_bioxus_api_crossplane_endpoints() {
    let app = setup_test_app();

    // 1. POST /api/v1/crossplane/claims
    let create_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/crossplane/claims")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "api_version": "database.servai.io/v1alpha1",
                        "kind": "PostgreSQLInstance",
                        "name": "bioxus-prod-db",
                        "namespace": "production",
                        "composition_ref": "postgres-ha",
                        "parameters": {
                            "size": "db.m6i.large",
                            "storage": "100Gi"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_res.status(), StatusCode::CREATED);
    let create_json: Value = serde_json::from_slice(
        &create_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(create_json["name"], "bioxus-prod-db");
    assert_eq!(create_json["kind"], "PostgreSQLInstance");
    assert_eq!(create_json["ready"], true);
    assert_eq!(create_json["synced"], true);
    assert_eq!(create_json["managed_resources"].as_array().unwrap().len(), 2);

    // 2. GET /api/v1/crossplane/claims/:namespace/:name
    let get_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/crossplane/claims/production/bioxus-prod-db")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_res.status(), StatusCode::OK);
    let get_json: Value = serde_json::from_slice(
        &get_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(get_json["name"], "bioxus-prod-db");
    assert_eq!(get_json["ready"], true);

    // 3. DELETE /api/v1/crossplane/claims/:namespace/:name
    let del_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/crossplane/claims/production/bioxus-prod-db")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(del_res.status(), StatusCode::NO_CONTENT);

    // 4. GET /api/v1/crossplane/resources
    let res_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/crossplane/resources?namespace=production")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res_res.status(), StatusCode::OK);
    let res_json: Value = serde_json::from_slice(
        &res_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(res_json.as_array().unwrap().len(), 2);
    assert_eq!(res_json[0]["provider"], "provider-aws");

    // 5. GET /api/v1/crossplane/providers
    let prov_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/crossplane/providers")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(prov_res.status(), StatusCode::OK);
    let prov_json: Value = serde_json::from_slice(
        &prov_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(prov_json.as_array().unwrap().len(), 3);
    assert_eq!(prov_json[0]["name"], "provider-aws");
}

#[tokio::test]
async fn test_bioxus_api_wireguard_endpoints() {
    let app = setup_test_app();

    // 1. POST /api/v1/wireguard/keypair
    let key_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/wireguard/keypair")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(key_res.status(), StatusCode::OK);
    let key_json: Value = serde_json::from_slice(
        &key_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    let privkey = key_json["private_key"].as_str().unwrap();
    let pubkey = key_json["public_key"].as_str().unwrap();
    assert!(!privkey.is_empty());
    assert!(!pubkey.is_empty());

    // 2. Register server and bring it online
    let srv_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/servers")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "wg-gateway",
                        "ip": "10.0.0.77",
                        "port": 22,
                        "credentials": "Agent",
                        "tags": ["wireguard"]
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

    // 3. POST /api/v1/servers/:id/wireguard (configure wg0)
    let cfg_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/servers/{}/wireguard", server_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "wg0",
                        "private_key": privkey,
                        "public_key": pubkey,
                        "listen_port": 51820,
                        "address": "10.100.0.1/24",
                        "peers": [
                            {
                                "public_key": "peer-initial-pubkey=",
                                "allowed_ips": ["10.100.0.2/32"]
                            }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(cfg_res.status(), StatusCode::CREATED);

    // 4. GET /api/v1/servers/:id/wireguard/wg0/status
    let status_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/servers/{}/wireguard/wg0/status", server_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(status_res.status(), StatusCode::OK);
    let status_json: Value = serde_json::from_slice(
        &status_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(status_json["interface"], "wg0");
    assert_eq!(status_json["active_peers"], 1);
    assert_eq!(status_json["is_up"], true);

    // 5. POST /api/v1/servers/:id/wireguard/wg0/peers (add peer)
    let add_peer_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/servers/{}/wireguard/wg0/peers", server_id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "public_key": "peer-roadwarrior-pubkey=",
                        "endpoint": "203.0.113.10:51820",
                        "allowed_ips": ["10.100.0.3/32"],
                        "persistent_keepalive": 25
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(add_peer_res.status(), StatusCode::CREATED);

    // 6. GET /api/v1/servers/:id/wireguard/wg0/peers
    let peers_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/servers/{}/wireguard/wg0/peers", server_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(peers_res.status(), StatusCode::OK);
    let peers_json: Value = serde_json::from_slice(
        &peers_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(peers_json.as_array().unwrap().len(), 2);

    // 7. DELETE /api/v1/servers/:id/wireguard/wg0/peers/:pubkey
    let del_peer_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/v1/servers/{}/wireguard/wg0/peers/peer-initial-pubkey=",
                    server_id
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(del_peer_res.status(), StatusCode::NO_CONTENT);

    // 8. Verify remaining peers
    let peers_final_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/servers/{}/wireguard/wg0/peers", server_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let peers_final_json: Value = serde_json::from_slice(
        &peers_final_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(peers_final_json.as_array().unwrap().len(), 1);
    assert_eq!(
        peers_final_json[0]["public_key"],
        "peer-roadwarrior-pubkey="
    );
}

#[tokio::test]
async fn test_bioxus_api_dns_and_registrar_endpoints() {
    let app = setup_test_app();

    // 1. Check domain availability (GET /api/v1/domains/check)
    let check_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/domains/check?domain=bioxus-platform.org")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(check_res.status(), StatusCode::OK);
    let check_json: Value = serde_json::from_slice(
        &check_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(check_json["domain"], "bioxus-platform.org");
    assert_eq!(check_json["available"], true);

    // 2. Register a domain (POST /api/v1/domains)
    let register_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/domains")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "domain": "bioxus-platform.org",
                        "provider": "Cloudflare",
                        "contact_name": "DevOps Lead",
                        "contact_email": "ops@bioxus.org",
                        "contact_phone": "+33123456789",
                        "contact_city": "Paris",
                        "contact_country": "France",
                        "nameservers": ["ns1.cloudflare.com", "ns2.cloudflare.com"],
                        "auto_renew": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(register_res.status(), StatusCode::CREATED);
    let reg_json: Value = serde_json::from_slice(
        &register_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(reg_json["domain"], "bioxus-platform.org");
    assert_eq!(reg_json["status"], "Active");
    assert_eq!(reg_json["auto_renew"], true);

    // 3. Update Nameservers (PUT /api/v1/domains/:domain/nameservers)
    let ns_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/domains/bioxus-platform.org/nameservers")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "nameservers": ["ns1.custom-dns.io", "ns2.custom-dns.io"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(ns_res.status(), StatusCode::OK);

    // 4. Toggle Auto-Renew (PUT /api/v1/domains/:domain/auto-renew)
    let renew_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/domains/bioxus-platform.org/auto-renew")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "enable": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(renew_res.status(), StatusCode::OK);

    // Verify domain updated state via GET /api/v1/domains/:domain
    let get_dom_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/domains/bioxus-platform.org")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_dom_res.status(), StatusCode::OK);
    let get_dom_json: Value = serde_json::from_slice(
        &get_dom_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(
        get_dom_json["nameservers"]["servers"][0],
        "ns1.custom-dns.io"
    );
    assert_eq!(get_dom_json["auto_renew"], false);

    // 5. Create DNS Zone (POST /api/v1/dns/zones)
    let zone_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/dns/zones")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "domain": "bioxus-platform.org",
                        "enable_dnssec": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(zone_res.status(), StatusCode::CREATED);
    let zone_json: Value = serde_json::from_slice(
        &zone_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(zone_json["domain"], "bioxus-platform.org");
    assert_eq!(zone_json["dnssec_status"], "Enabled");

    // 6. Upsert DNS A-Record (POST /api/v1/dns/zones/:domain/records)
    let record_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/dns/zones/bioxus-platform.org/records")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "api",
                        "record_type": "A",
                        "value": "198.51.100.25",
                        "ttl_seconds": 300
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(record_res.status(), StatusCode::CREATED);
    let record_json: Value = serde_json::from_slice(
        &record_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    let record_id = record_json["id"].as_str().unwrap().to_string();
    assert_eq!(record_json["name"], "api");
    assert_eq!(record_json["record_type"], "A");
    assert_eq!(record_json["value"], "198.51.100.25");

    // 7. Verify DNS Propagation (GET /api/v1/dns/zones/:domain/propagate)
    let prop_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/dns/zones/bioxus-platform.org/propagate?record_type=A&expected_value=198.51.100.25")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(prop_res.status(), StatusCode::OK);
    let prop_json: Value = serde_json::from_slice(
        &prop_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(prop_json["domain"], "bioxus-platform.org");
    assert_eq!(prop_json["propagated"], true);

    // 8. Delete DNS Record (DELETE /api/v1/dns/zones/:domain/records/:id)
    let del_rec_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/v1/dns/zones/bioxus-platform.org/records/{}",
                    record_id
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(del_rec_res.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_bioxus_api_bind_server_domain_and_auto_tls() {
    let app = setup_test_app();

    // 1. Register a server
    let reg_server_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/servers")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "prod-app-server",
                        "ip": "203.0.113.88",
                        "port": 22,
                        "credentials": "Agent"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(reg_server_res.status(), StatusCode::CREATED);
    let server_json: Value = serde_json::from_slice(
        &reg_server_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    let server_id = server_json["id"].as_str().unwrap();

    // 2. Bind server to domain (POST /api/v1/servers/:id/bind-domain)
    let bind_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/servers/{}/bind-domain", server_id))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "domain": "app.bioxus.cloud"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(bind_res.status(), StatusCode::CREATED);
    let cert_json: Value = serde_json::from_slice(
        &bind_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(cert_json["primary_domain"], "app.bioxus.cloud");
    assert_eq!(cert_json["status"], "Valid");
    assert_eq!(cert_json["challenge_type"], "Dns01");
    assert_eq!(cert_json["days_remaining"], 90);
}

#[tokio::test]
async fn test_bioxus_api_complete_orchestration_endpoints() {
    let app = setup_test_app();

    // 1. Register a server and bring online
    let s_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/servers")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "orch-node-01",
                        "ip": "10.0.1.20",
                        "port": 22,
                        "credentials": "Agent",
                        "tags": ["web", "core"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(s_res.status(), StatusCode::CREATED);
    let s_json: Value = serde_json::from_slice(
        &s_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    let server_id = s_json["id"].as_str().unwrap().to_string();

    let _ = app
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

    // 2. Register a repository
    let repo_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/repos")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "base-hardening",
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

    assert_eq!(repo_res.status(), StatusCode::CREATED);
    let repo_json: Value = serde_json::from_slice(
        &repo_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    let repo_id = repo_json["id"].as_str().unwrap().to_string();

    // 3. Trigger orchestration (POST /api/v1/orchestrations)
    let trig_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/orchestrations")
                .header("Content-Type", "application/json")
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

    assert_eq!(trig_res.status(), StatusCode::CREATED);
    let job_json: Value = serde_json::from_slice(
        &trig_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    let job_id = job_json["id"].as_str().unwrap().to_string();

    // 4. List orchestrations (GET /api/v1/orchestrations)
    let list_jobs_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/orchestrations")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(list_jobs_res.status(), StatusCode::OK);
    let jobs_arr: Value = serde_json::from_slice(
        &list_jobs_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert!(!jobs_arr.as_array().unwrap().is_empty());

    // 5. Get orchestration logs (GET /api/v1/orchestrations/:id/logs)
    let logs_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/orchestrations/{}/logs", job_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(logs_res.status(), StatusCode::OK);
    let logs_json: Value = serde_json::from_slice(
        &logs_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(logs_json["job_id"], job_id);

    // 6. Create workflow (POST /api/v1/workflows)
    let wf_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflows")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "full-stack-bootstrap",
                        "description": "Bootstrap and configure all services",
                        "steps": [
                            {
                                "name": "disk-check",
                                "description": "Ensure disk space",
                                "action": {
                                    "ShellCommand": {
                                        "command": "df -h"
                                    }
                                }
                            }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(wf_res.status(), StatusCode::CREATED);
    let wf_json: Value = serde_json::from_slice(
        &wf_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    let wf_id = wf_json["id"].as_str().unwrap().to_string();

    // 7. List workflows (GET /api/v1/workflows)
    let list_wf_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/workflows")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(list_wf_res.status(), StatusCode::OK);

    // 8. Execute workflow (POST /api/v1/workflows/:id/execute)
    let exec_wf_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/workflows/{}/execute", wf_id))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "server_id": server_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(exec_wf_res.status(), StatusCode::CREATED);

    // 9. Fleet rollout (POST /api/v1/orchestrations/fleet)
    let fleet_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/orchestrations/fleet")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "target_tag": "web",
                        "server_ids": [],
                        "strategy": "Parallel",
                        "action": {
                            "ShellCommand": {
                                "command": "uname -a"
                            }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(fleet_res.status(), StatusCode::OK);
    let fleet_json: Value = serde_json::from_slice(
        &fleet_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(fleet_json["succeeded_count"], 1);

    // 10. Create schedule (POST /api/v1/schedules)
    let sched_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/schedules")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "daily-telemetry-cleanup",
                        "cron_expression": "0 3 * * *",
                        "server_id": server_id,
                        "action": {
                            "ShellCommand": {
                                "command": "journalctl --vacuum-time=7d"
                            }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(sched_res.status(), StatusCode::CREATED);
    let sched_json: Value = serde_json::from_slice(
        &sched_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    let sched_id = sched_json["id"].as_str().unwrap().to_string();

    // 11. Run schedule now (POST /api/v1/schedules/:id/run)
    let run_sched_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/schedules/{}/run", sched_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(run_sched_res.status(), StatusCode::CREATED);

    // 12. Delete schedule (DELETE /api/v1/schedules/:id)
    let del_sched_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/schedules/{}", sched_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(del_sched_res.status(), StatusCode::NO_CONTENT);

    // 13. Reconcile server state (POST /api/v1/orchestrations/reconcile)
    let recon_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/orchestrations/reconcile")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "server_id": server_id,
                        "required_packages": ["dev-sec-hardening"],
                        "bound_domain": "app.bioxus.cloud",
                        "wireguard_enabled": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(recon_res.status(), StatusCode::OK);
    let recon_json: Value = serde_json::from_slice(
        &recon_res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(recon_json["server_id"], server_id);
    assert!(recon_json["drift_detected"].as_bool().unwrap());
    assert_eq!(recon_json["remediation_jobs"].as_array().unwrap().len(), 3);
}





