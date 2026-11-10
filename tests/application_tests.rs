use std::sync::Arc;
use servai::{
    adapters::outbound::{
        memory::{
            job_store::InMemoryJobStore, repo_store::InMemoryRepoStore,
            server_store::InMemoryServerStore,
        },
        mock::{MockGitProvider, MockServerExecutor},
    },
    application::{
        orchestration_service::OrchestrationService,
        server_service::{RegisterServerCommand, ServerService},
    },
    domain::{
        errors::DomainError,
        orchestration::ExecutionStatus,
        repository::{GitRef, RepoKind, SyncStatus},
        server::{ServerStatus, SshCredentials},
    },
    ports::inbound::{
        repo_orchestrator::{RegisterRepoCommand, RepoOrchestratorPort},
        server_admin::ServerAdminPort,
    },
};

#[tokio::test]
async fn test_server_service_registration_and_healthcheck() {
    let store = Arc::new(InMemoryServerStore::default());
    let executor = Arc::new(MockServerExecutor::default());
    let service = ServerService::new(store.clone(), executor.clone());

    let cmd = RegisterServerCommand {
        name: "node-primary".to_string(),
        ip: "10.0.0.5".to_string(),
        port: 22,
        credentials: SshCredentials::Agent,
        tags: vec!["production".to_string(), "api".to_string()],
    };

    let server = service.register_server(cmd).await.expect("registration succeeds");
    assert_eq!(server.name().as_str(), "node-primary");
    assert_eq!(server.tags(), &["production", "api"]);
    assert_eq!(server.status(), ServerStatus::Pending);

    // Run healthcheck: executor ping returns true -> server transitions to Online
    let status = service.health_check(&server.id()).await.expect("healthcheck succeeds");
    assert_eq!(status, ServerStatus::Online);

    let fetched = service.get_server(&server.id()).await.expect("get succeeds");
    assert_eq!(fetched.status(), ServerStatus::Online);
    // Specs should be populated by inspect_system
    assert!(fetched.spec().is_some());
}

#[tokio::test]
async fn test_server_service_unreachable_node_goes_offline() {
    let store = Arc::new(InMemoryServerStore::default());
    let executor = Arc::new(MockServerExecutor {
        should_fail_ping: true,
        ..Default::default()
    });
    let service = ServerService::new(store.clone(), executor.clone());

    let cmd = RegisterServerCommand {
        name: "node-faulty".to_string(),
        ip: "10.0.0.99".to_string(),
        port: 22,
        credentials: SshCredentials::Agent,
        tags: vec![],
    };

    let server = service.register_server(cmd).await.unwrap();
    let status = service.health_check(&server.id()).await.unwrap();
    assert_eq!(status, ServerStatus::Offline);
}

#[tokio::test]
async fn test_orchestration_service_happy_path() {
    let server_store = Arc::new(InMemoryServerStore::default());
    let repo_store = Arc::new(InMemoryRepoStore::default());
    let job_store = Arc::new(InMemoryJobStore::default());
    let executor = Arc::new(MockServerExecutor::default());
    let git = Arc::new(MockGitProvider::default());

    let server_service = ServerService::new(server_store.clone(), executor.clone());
    let orchestrator = OrchestrationService::new(
        server_store.clone(),
        repo_store.clone(),
        job_store.clone(),
        executor.clone(),
        git.clone(),
    );

    // Register and activate server
    let server = server_service
        .register_server(RegisterServerCommand {
            name: "srv-01".to_string(),
            ip: "192.168.1.10".to_string(),
            port: 22,
            credentials: SshCredentials::Agent,
            tags: vec![],
        })
        .await
        .unwrap();
    server_service.health_check(&server.id()).await.unwrap();

    // Register repo
    let repo = orchestrator
        .register_repo(RegisterRepoCommand {
            name: "nginx-deployments".to_string(),
            url: "https://github.com/org/nginx-deployments.git".to_string(),
            target_ref: GitRef::Branch("main".to_string()),
            kind: RepoKind::DockerCompose,
        })
        .await
        .unwrap();

    // Sync repo
    let synced_repo = orchestrator.sync_repo(&repo.id()).await.unwrap();
    assert_eq!(synced_repo.sync_status(), SyncStatus::Synced);
    assert_eq!(synced_repo.last_commit_hash(), Some("abcdef1234567890"));

    // Trigger orchestration
    let job = orchestrator
        .trigger_orchestration(&server.id(), &repo.id())
        .await
        .unwrap();

    assert_eq!(job.status(), ExecutionStatus::Succeeded);
    assert_eq!(job.steps().len(), 2);
}

#[tokio::test]
async fn test_orchestration_service_server_offline_rejected() {
    let server_store = Arc::new(InMemoryServerStore::default());
    let repo_store = Arc::new(InMemoryRepoStore::default());
    let job_store = Arc::new(InMemoryJobStore::default());
    let executor = Arc::new(MockServerExecutor {
        should_fail_ping: true,
        ..Default::default()
    });
    let git = Arc::new(MockGitProvider::default());

    let server_service = ServerService::new(server_store.clone(), executor.clone());
    let orchestrator = OrchestrationService::new(
        server_store.clone(),
        repo_store.clone(),
        job_store.clone(),
        executor.clone(),
        git.clone(),
    );

    let server = server_service
        .register_server(RegisterServerCommand {
            name: "offline-server".to_string(),
            ip: "10.0.0.12".to_string(),
            port: 22,
            credentials: SshCredentials::Agent,
            tags: vec![],
        })
        .await
        .unwrap();
    server_service.health_check(&server.id()).await.unwrap();

    let repo = orchestrator
        .register_repo(RegisterRepoCommand {
            name: "app-manifest".to_string(),
            url: "https://github.com/org/app.git".to_string(),
            target_ref: GitRef::Branch("main".to_string()),
            kind: RepoKind::Playbook,
        })
        .await
        .unwrap();

    // Attempting orchestration on an Offline server must fail with Conflict
    let err = orchestrator
        .trigger_orchestration(&server.id(), &repo.id())
        .await
        .unwrap_err();

    assert!(matches!(err, DomainError::Conflict(_)));
}

#[tokio::test]
async fn test_server_service_manage_systemd_service() {
    use servai::domain::server::{ServiceAction, ServiceStatus};

    let store = Arc::new(InMemoryServerStore::default());
    let executor = Arc::new(MockServerExecutor::default());
    let service = ServerService::new(store.clone(), executor.clone());

    let server = service
        .register_server(RegisterServerCommand {
            name: "web-01".to_string(),
            ip: "10.0.0.1".to_string(),
            port: 22,
            credentials: SshCredentials::Agent,
            tags: vec![],
        })
        .await
        .unwrap();
    service.health_check(&server.id()).await.unwrap();

    let status = service
        .manage_service(&server.id(), "nginx", ServiceAction::Restart)
        .await
        .unwrap();

    assert_eq!(status, ServiceStatus::Active);
}

#[tokio::test]
async fn test_server_service_collect_metrics() {
    let store = Arc::new(InMemoryServerStore::default());
    let executor = Arc::new(MockServerExecutor::default());
    let service = ServerService::new(store.clone(), executor.clone());

    let server = service
        .register_server(RegisterServerCommand {
            name: "metrics-node".to_string(),
            ip: "10.0.0.2".to_string(),
            port: 22,
            credentials: SshCredentials::Agent,
            tags: vec![],
        })
        .await
        .unwrap();
    service.health_check(&server.id()).await.unwrap();

    let metrics = service.get_metrics(&server.id()).await.unwrap();
    assert!(metrics.cpu_usage_pct >= 0.0);
    assert!(metrics.ram_used_mb <= metrics.ram_total_mb);
}

#[tokio::test]
async fn test_server_service_reboot() {
    let store = Arc::new(InMemoryServerStore::default());
    let executor = Arc::new(MockServerExecutor::default());
    let service = ServerService::new(store.clone(), executor.clone());

    let server = service
        .register_server(RegisterServerCommand {
            name: "reboot-node".to_string(),
            ip: "10.0.0.3".to_string(),
            port: 22,
            credentials: SshCredentials::Agent,
            tags: vec![],
        })
        .await
        .unwrap();
    service.health_check(&server.id()).await.unwrap();

    service.reboot_server(&server.id()).await.unwrap();

    let after_reboot = service.get_server(&server.id()).await.unwrap();
    assert_eq!(after_reboot.status(), ServerStatus::Maintenance);
}

#[tokio::test]
async fn test_orchestration_service_rollback_on_step_failure() {
    use servai::domain::orchestration::RollbackPolicy;

    let server_store = Arc::new(InMemoryServerStore::default());
    let repo_store = Arc::new(InMemoryRepoStore::default());
    let job_store = Arc::new(InMemoryJobStore::default());
    let executor = Arc::new(MockServerExecutor::default());
    let git = Arc::new(MockGitProvider::default());

    let server_service = ServerService::new(server_store.clone(), executor.clone());
    let orchestrator = OrchestrationService::new(
        server_store.clone(),
        repo_store.clone(),
        job_store.clone(),
        executor.clone(),
        git.clone(),
    );

    let server = server_service
        .register_server(RegisterServerCommand {
            name: "app-srv".to_string(),
            ip: "10.0.0.4".to_string(),
            port: 22,
            credentials: SshCredentials::Agent,
            tags: vec![],
        })
        .await
        .unwrap();
    server_service.health_check(&server.id()).await.unwrap();

    let repo = orchestrator
        .register_repo(RegisterRepoCommand {
            name: "app-repo".to_string(),
            url: "https://github.com/org/app.git".to_string(),
            target_ref: GitRef::Branch("main".to_string()),
            kind: RepoKind::Playbook,
        })
        .await
        .unwrap();

    let job = orchestrator
        .trigger_orchestration_with_options(
            &server.id(),
            &repo.id(),
            RollbackPolicy::RollbackOnFailure,
        )
        .await
        .unwrap();

    assert_eq!(job.status(), ExecutionStatus::Succeeded);

    // Verify server lock is freed after job finishes
    let s = server_service.get_server(&server.id()).await.unwrap();
    assert!(!s.is_locked());
}

#[tokio::test]
async fn test_orchestration_service_drift_detection() {
    let server_store = Arc::new(InMemoryServerStore::default());
    let repo_store = Arc::new(InMemoryRepoStore::default());
    let job_store = Arc::new(InMemoryJobStore::default());
    let executor = Arc::new(MockServerExecutor::default());
    let git = Arc::new(MockGitProvider::default());

    let server_service = ServerService::new(server_store.clone(), executor.clone());
    let orchestrator = OrchestrationService::new(
        server_store.clone(),
        repo_store.clone(),
        job_store.clone(),
        executor.clone(),
        git.clone(),
    );

    let server = server_service
        .register_server(RegisterServerCommand {
            name: "drift-node".to_string(),
            ip: "10.0.0.5".to_string(),
            port: 22,
            credentials: SshCredentials::Agent,
            tags: vec![],
        })
        .await
        .unwrap();
    server_service.health_check(&server.id()).await.unwrap();

    let repo = orchestrator
        .register_repo(RegisterRepoCommand {
            name: "drift-repo".to_string(),
            url: "https://github.com/org/drift.git".to_string(),
            target_ref: GitRef::Branch("main".to_string()),
            kind: RepoKind::RawConfig,
        })
        .await
        .unwrap();

    let drift = orchestrator.detect_drift(&server.id(), &repo.id()).await.unwrap();
    assert!(!drift.has_drift());
}
