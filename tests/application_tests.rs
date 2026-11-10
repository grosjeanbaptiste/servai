use std::collections::HashMap;
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
    ports::{
        inbound::{
            repo_orchestrator::{RegisterRepoCommand, RepoOrchestratorPort},
            server_admin::ServerAdminPort,
        },
        outbound::repo_store::RepoStorePort,
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

#[tokio::test]
async fn test_orchestration_service_browse_and_import_admin_catalog() {
    use servai::domain::catalog::AdminCategory;
    use servai::adapters::outbound::catalog::StandardServerAdminCatalog;

    let server_store = Arc::new(InMemoryServerStore::default());
    let repo_store = Arc::new(InMemoryRepoStore::default());
    let job_store = Arc::new(InMemoryJobStore::default());
    let executor = Arc::new(MockServerExecutor::default());
    let git = Arc::new(MockGitProvider::default());
    let catalog = Arc::new(StandardServerAdminCatalog::default());

    let orchestrator = OrchestrationService::new_with_catalog(
        server_store,
        repo_store.clone(),
        job_store,
        executor,
        git,
        catalog,
    );

    // List all server administration repositories in the catalog
    let all_items = orchestrator.list_catalog(None).await.unwrap();
    assert!(all_items.len() >= 6);

    // Filter by Security category
    let security_items = orchestrator
        .list_catalog(Some(AdminCategory::Security))
        .await
        .unwrap();
    assert!(!security_items.is_empty());
    assert!(security_items.iter().any(|item| item.id() == "linux-hardening"));

    // Import the linux-hardening repo into managed repos
    let imported_repo = orchestrator
        .import_from_catalog("linux-hardening", Some("my-company-hardening".to_string()))
        .await
        .unwrap();

    assert_eq!(imported_repo.name(), "my-company-hardening");
    assert_eq!(
        imported_repo.url(),
        "https://github.com/dev-sec/ansible-collection-hardening.git"
    );

    // Verify it is now saved in the repo store
    let repos = repo_store.list().await.unwrap();
    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0].id(), imported_repo.id());
}

#[tokio::test]
async fn test_orchestration_service_apply_recipe_directly_to_server() {
    use servai::adapters::outbound::catalog::StandardServerAdminCatalog;
    use std::collections::HashMap;

    let server_store = Arc::new(InMemoryServerStore::default());
    let repo_store = Arc::new(InMemoryRepoStore::default());
    let job_store = Arc::new(InMemoryJobStore::default());
    let executor = Arc::new(MockServerExecutor::default());
    let git = Arc::new(MockGitProvider::default());
    let catalog = Arc::new(StandardServerAdminCatalog::default());

    let server_service = ServerService::new(server_store.clone(), executor.clone());
    let orchestrator = OrchestrationService::new_with_catalog(
        server_store.clone(),
        repo_store.clone(),
        job_store.clone(),
        executor.clone(),
        git.clone(),
        catalog,
    );

    // Register & healthcheck server (Ubuntu)
    let server = server_service
        .register_server(RegisterServerCommand {
            name: "prod-bastion".to_string(),
            ip: "10.0.0.88".to_string(),
            port: 22,
            credentials: SshCredentials::Agent,
            tags: vec!["bastion".to_string()],
        })
        .await
        .unwrap();
    server_service.health_check(&server.id()).await.unwrap();

    let mut params = HashMap::new();
    params.insert("ssh_port".to_string(), "2222".to_string());

    // Apply "linux-hardening" recipe directly
    let job = orchestrator
        .apply_recipe(&server.id(), "linux-hardening", params)
        .await
        .unwrap();

    assert_eq!(job.status(), ExecutionStatus::Succeeded);
    assert!(!job.steps().is_empty());
}

#[tokio::test]
async fn test_orchestration_service_multi_repo_stack_execution() {
    use servai::domain::stack::StackRepoEntry;

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

    // Register server
    let server = server_service
        .register_server(RegisterServerCommand {
            name: "full-stack-srv".to_string(),
            ip: "10.0.0.9".to_string(),
            port: 22,
            credentials: SshCredentials::Agent,
            tags: vec![],
        })
        .await
        .unwrap();
    server_service.health_check(&server.id()).await.unwrap();

    // Register two distinct server administration repositories
    let repo_sec = orchestrator
        .register_repo(RegisterRepoCommand {
            name: "admin-security".to_string(),
            url: "https://github.com/dev-sec/ansible-collection-hardening.git".to_string(),
            target_ref: GitRef::Branch("main".to_string()),
            kind: RepoKind::Playbook,
        })
        .await
        .unwrap();

    let repo_docker = orchestrator
        .register_repo(RegisterRepoCommand {
            name: "admin-docker".to_string(),
            url: "https://github.com/geerlingguy/ansible-role-docker.git".to_string(),
            target_ref: GitRef::Branch("master".to_string()),
            kind: RepoKind::Playbook,
        })
        .await
        .unwrap();

    // Compose them into a single multi-repo administration stack
    let stack = orchestrator
        .create_stack(
            "production-baseline-stack".to_string(),
            "Chains OS hardening and Docker runtime deployment".to_string(),
            vec![
                StackRepoEntry {
                    repo_id: repo_sec.id(),
                    order: 1,
                    step_name: "apply-hardening".to_string(),
                    command: "ansible-playbook hardening.yml".to_string(),
                    rollback_command: None,
                    overrides: HashMap::new(),
                },
                StackRepoEntry {
                    repo_id: repo_docker.id(),
                    order: 2,
                    step_name: "install-docker".to_string(),
                    command: "ansible-playbook docker.yml".to_string(),
                    rollback_command: None,
                    overrides: HashMap::new(),
                },
            ],
        )
        .await
        .unwrap();

    // Execute the composite multi-repo stack on the server
    let job = orchestrator
        .execute_stack(&server.id(), &stack.id())
        .await
        .unwrap();

    assert_eq!(job.status(), ExecutionStatus::Succeeded);
    assert_eq!(job.steps().len(), 2);
    assert_eq!(job.steps()[0].step.name, "apply-hardening");
    assert_eq!(job.steps()[1].step.name, "install-docker");
}

#[tokio::test]
async fn test_orchestration_service_search_registry_and_dependency_resolution() {
    use servai::domain::catalog::AdminCategory;
    use servai::domain::registry::RegistryQuery;

    let server_store = Arc::new(InMemoryServerStore::default());
    let repo_store = Arc::new(InMemoryRepoStore::default());
    let job_store = Arc::new(InMemoryJobStore::default());
    let executor = Arc::new(MockServerExecutor::default());
    let git = Arc::new(MockGitProvider::default());

    let orchestrator = OrchestrationService::new(
        server_store,
        repo_store,
        job_store,
        executor,
        git,
    );

    // Search for container packages
    let query = RegistryQuery::builder()
        .category(AdminCategory::Containers)
        .build();

    let packages = orchestrator.search_registry(query).await.unwrap();
    assert!(!packages.is_empty());
    assert!(packages.iter().any(|p| p.id() == "geerlingguy-docker"));

    // List server profile presets
    let presets = orchestrator.list_profile_presets().await.unwrap();
    assert!(presets.len() >= 4);
    assert!(presets.iter().any(|p| p.id() == "secure-web-baseline"));
}

#[tokio::test]
async fn test_orchestration_service_install_package_with_auto_dependencies() {
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
            name: "wireguard-host".to_string(),
            ip: "10.0.0.33".to_string(),
            port: 22,
            credentials: SshCredentials::Agent,
            tags: vec![],
        })
        .await
        .unwrap();
    server_service.health_check(&server.id()).await.unwrap();

    // "linuxserver-wireguard" depends on "geerlingguy-docker"
    // Orchestrator must resolve and execute both in dependency order!
    let job = orchestrator
        .install_package_with_dependencies(&server.id(), "linuxserver-wireguard", HashMap::new())
        .await
        .unwrap();

    assert_eq!(job.status(), ExecutionStatus::Succeeded);
    assert_eq!(job.steps().len(), 2);
    assert_eq!(job.steps()[0].step.name, "install-geerlingguy-docker");
    assert_eq!(job.steps()[1].step.name, "install-linuxserver-wireguard");
}

#[tokio::test]
async fn test_orchestration_service_apply_profile_preset() {
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
            name: "web-baseline-srv".to_string(),
            ip: "10.0.0.44".to_string(),
            port: 22,
            credentials: SshCredentials::Agent,
            tags: vec![],
        })
        .await
        .unwrap();
    server_service.health_check(&server.id()).await.unwrap();

    // Apply "secure-web-baseline" preset (bundles 3 admin packages)
    let job = orchestrator
        .apply_profile_preset(&server.id(), "secure-web-baseline")
        .await
        .unwrap();

    assert_eq!(job.status(), ExecutionStatus::Succeeded);
    assert_eq!(job.steps().len(), 3);
}



