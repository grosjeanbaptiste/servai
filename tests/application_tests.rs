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

#[tokio::test]
async fn test_orchestration_service_opentofu_plan_apply_destroy_and_state() {
    let server_store = Arc::new(InMemoryServerStore::default());
    let repo_store = Arc::new(InMemoryRepoStore::default());
    let job_store = Arc::new(InMemoryJobStore::default());
    let executor = Arc::new(MockServerExecutor::default());
    let git = Arc::new(MockGitProvider::default());

    let orchestrator = OrchestrationService::new(
        server_store.clone(),
        repo_store.clone(),
        job_store.clone(),
        executor.clone(),
        git.clone(),
    );

    let repo = orchestrator
        .register_repo(RegisterRepoCommand {
            name: "infra-cluster".to_string(),
            url: "https://github.com/org/infra-cluster.git".to_string(),
            target_ref: GitRef::Branch("main".to_string()),
            kind: RepoKind::OpenTofu,
        })
        .await
        .unwrap();

    let mut vars = HashMap::new();
    vars.insert("node_count".to_string(), "2".to_string());

    // 1. Plan
    let plan = orchestrator.plan_tofu(&repo.id(), vars.clone()).await.unwrap();
    assert_eq!(plan.summary().to_add, 2);
    assert_eq!(plan.summary().to_change, 0);
    assert_eq!(plan.summary().to_destroy, 0);
    assert_eq!(plan.resources().len(), 2);

    // 2. Apply
    let apply = orchestrator.apply_tofu(&repo.id(), vars.clone()).await.unwrap();
    assert!(apply.is_success());
    assert_eq!(apply.resources_created(), 2);
    assert_eq!(
        apply.outputs().get("cluster_endpoint").unwrap(),
        "https://10.0.0.10:6443"
    );

    // 3. State
    let state = orchestrator.get_tofu_state(&repo.id()).await.unwrap();
    assert_eq!(state.serial(), 1);
    assert_eq!(state.lineage(), "mock-lineage-123");
    assert_eq!(state.resources().len(), 2);

    // 4. Destroy
    let destroy = orchestrator.destroy_tofu(&repo.id(), vars).await.unwrap();
    assert!(destroy.is_success());
    assert_eq!(destroy.resources_deleted(), 2);
}

#[tokio::test]
async fn test_orchestration_service_ansible_playbook() {
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
            name: "ansible-target".to_string(),
            ip: "10.0.0.99".to_string(),
            port: 22,
            credentials: SshCredentials::Agent,
            tags: vec![],
        })
        .await
        .unwrap();
    server_service.health_check(&server.id()).await.unwrap();

    let mut vars = HashMap::new();
    vars.insert("app_port".to_string(), "8080".to_string());

    let result = orchestrator
        .run_ansible_playbook(&server.id(), "site.yml", vars)
        .await
        .unwrap();

    assert!(result.is_successful());
    assert_eq!(result.ok_count(), 3);
    assert_eq!(result.changed_count(), 1);
    assert_eq!(result.failed_count(), 0);
    assert_eq!(result.task_events().len(), 3);
}

#[tokio::test]
async fn test_server_service_ansible_module_and_facts() {
    let server_store = Arc::new(InMemoryServerStore::default());
    let executor = Arc::new(MockServerExecutor::default());
    let server_service = ServerService::new(server_store.clone(), executor.clone());

    let server = server_service
        .register_server(RegisterServerCommand {
            name: "ansible-adhoc-node".to_string(),
            ip: "10.0.0.100".to_string(),
            port: 22,
            credentials: SshCredentials::Agent,
            tags: vec![],
        })
        .await
        .unwrap();
    server_service.health_check(&server.id()).await.unwrap();

    // Ad-hoc module execution
    let module_res = server_service
        .run_ansible_module(&server.id(), "command", "uptime")
        .await
        .unwrap();
    assert!(module_res.is_success());
    assert!(module_res.changed);
    assert_eq!(module_res.module, "command");

    // Fact gathering
    let facts = server_service.gather_ansible_facts(&server.id()).await.unwrap();
    assert_eq!(facts.distribution, "Ubuntu");
    assert_eq!(facts.distribution_version, "22.04");
    assert_eq!(facts.memtotal_mb, 8192);
    assert!(facts.interfaces.contains(&"eth0".to_string()));
}

#[tokio::test]
async fn test_orchestration_service_crossplane_claim_lifecycle() {
    use servai::domain::engines::crossplane::CrossplaneClaim;

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

    let mut params = HashMap::new();
    params.insert("storageGb".to_string(), "50".to_string());

    let claim = CrossplaneClaim::new(
        "database.example.org/v1alpha1",
        "PostgreSQLInstance",
        "app-db",
        "staging",
        Some("postgres-composition".to_string()),
        params,
    )
    .unwrap();

    // 1. Apply claim
    let result = orchestrator.apply_crossplane_claim(claim).await.unwrap();
    assert!(result.is_ready());
    assert!(result.is_synced());
    assert_eq!(result.name(), "app-db");
    assert_eq!(result.managed_resources().len(), 2);

    // 2. Query claim status
    let status = orchestrator.get_crossplane_claim_status("staging", "app-db").await.unwrap();
    assert!(status.is_ready());
    assert_eq!(status.name(), "app-db");

    // 3. Delete claim
    let delete_res = orchestrator.delete_crossplane_claim("staging", "app-db").await;
    assert!(delete_res.is_ok());
}

#[tokio::test]
async fn test_orchestration_service_crossplane_providers_and_resources() {
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

    // List providers
    let providers = orchestrator.list_crossplane_providers().await.unwrap();
    assert_eq!(providers.len(), 3);
    assert_eq!(providers[0].name, "provider-aws");
    assert!(providers[0].installed && providers[0].healthy);

    // List managed resources
    let resources = orchestrator.list_crossplane_resources(Some("default".to_string())).await.unwrap();
    assert_eq!(resources.len(), 2);
    assert!(resources[0].ready && resources[0].synced);
    assert_eq!(resources[0].provider, "provider-aws");
}





