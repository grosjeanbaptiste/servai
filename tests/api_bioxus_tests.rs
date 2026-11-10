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
