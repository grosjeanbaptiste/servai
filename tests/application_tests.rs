use async_trait::async_trait;
use servai::{
    application::{
        orchestration_service::OrchestrationService,
        server_service::{RegisterServerCommand, ServerService},
    },
    domain::{
        errors::DomainError,
        orchestration::{ExecutionStatus, JobId, OrchestrationJob, OrchestrationStep},
        repository::{GitRef, ManagedRepository, RepoId, RepoKind, SyncStatus},
        server::{Server, ServerId, ServerSpec, ServerStatus, SshCredentials},
    },
    ports::{
        inbound::{
            repo_orchestrator::{RegisterRepoCommand, RepoOrchestratorPort},
            server_admin::ServerAdminPort,
        },
        outbound::{
            git_provider::GitProviderPort,
            job_store::JobStorePort,
            repo_store::RepoStorePort,
            server_executor::{CommandResult, ServerExecutorPort},
            server_store::ServerStorePort,
        },
    },
};
use std::sync::{Arc, Mutex};

// In-Memory Test Doubles for Outbound Ports
#[derive(Default, Clone)]
struct InMemoryServerStore {
    servers: Arc<Mutex<Vec<Server>>>,
}

#[async_trait]
impl ServerStorePort for InMemoryServerStore {
    async fn save(&self, server: Server) -> Result<(), DomainError> {
        let mut list = self.servers.lock().unwrap();
        if let Some(pos) = list.iter().position(|s| s.id() == server.id()) {
            list[pos] = server;
        } else {
            list.push(server);
        }
        Ok(())
    }

    async fn find_by_id(&self, id: &ServerId) -> Result<Option<Server>, DomainError> {
        let list = self.servers.lock().unwrap();
        Ok(list.iter().find(|s| s.id() == *id).cloned())
    }

    async fn list(&self) -> Result<Vec<Server>, DomainError> {
        let list = self.servers.lock().unwrap();
        Ok(list.clone())
    }

    async fn delete(&self, id: &ServerId) -> Result<(), DomainError> {
        let mut list = self.servers.lock().unwrap();
        list.retain(|s| s.id() != *id);
        Ok(())
    }
}

#[derive(Default, Clone)]
struct InMemoryRepoStore {
    repos: Arc<Mutex<Vec<ManagedRepository>>>,
}

#[async_trait]
impl RepoStorePort for InMemoryRepoStore {
    async fn save(&self, repo: ManagedRepository) -> Result<(), DomainError> {
        let mut list = self.repos.lock().unwrap();
        if let Some(pos) = list.iter().position(|r| r.id() == repo.id()) {
            list[pos] = repo;
        } else {
            list.push(repo);
        }
        Ok(())
    }

    async fn find_by_id(&self, id: &RepoId) -> Result<Option<ManagedRepository>, DomainError> {
        let list = self.repos.lock().unwrap();
        Ok(list.iter().find(|r| r.id() == *id).cloned())
    }

    async fn list(&self) -> Result<Vec<ManagedRepository>, DomainError> {
        let list = self.repos.lock().unwrap();
        Ok(list.clone())
    }
}

#[derive(Default, Clone)]
struct InMemoryJobStore {
    jobs: Arc<Mutex<Vec<OrchestrationJob>>>,
}

#[async_trait]
impl JobStorePort for InMemoryJobStore {
    async fn save(&self, job: OrchestrationJob) -> Result<(), DomainError> {
        let mut list = self.jobs.lock().unwrap();
        if let Some(pos) = list.iter().position(|j| j.id() == job.id()) {
            list[pos] = job;
        } else {
            list.push(job);
        }
        Ok(())
    }

    async fn find_by_id(&self, id: &JobId) -> Result<Option<OrchestrationJob>, DomainError> {
        let list = self.jobs.lock().unwrap();
        Ok(list.iter().find(|j| j.id() == *id).cloned())
    }

    async fn list_by_server(&self, server_id: &ServerId) -> Result<Vec<OrchestrationJob>, DomainError> {
        let list = self.jobs.lock().unwrap();
        Ok(list.iter().filter(|j| j.server_id() == *server_id).cloned().collect())
    }
}

#[derive(Clone)]
struct MockServerExecutor {
    should_fail_ping: bool,
    command_output: String,
    should_fail_command: bool,
}

impl Default for MockServerExecutor {
    fn default() -> Self {
        Self {
            should_fail_ping: false,
            command_output: "Success".to_string(),
            should_fail_command: false,
        }
    }
}

#[async_trait]
impl ServerExecutorPort for MockServerExecutor {
    async fn ping(&self, _server: &Server) -> Result<bool, DomainError> {
        if self.should_fail_ping {
            Ok(false)
        } else {
            Ok(true)
        }
    }

    async fn execute_command(&self, _server: &Server, _command: &str) -> Result<CommandResult, DomainError> {
        if self.should_fail_command {
            Err(DomainError::ExecutionFailed {
                reason: "Command returned exit code 1".to_string(),
                details: Some("Mock error output".to_string()),
            })
        } else {
            Ok(CommandResult {
                exit_code: 0,
                stdout: self.command_output.clone(),
                stderr: String::new(),
            })
        }
    }

    async fn inspect_system(&self, _server: &Server) -> Result<ServerSpec, DomainError> {
        Ok(ServerSpec {
            os: "Debian 12".to_string(),
            arch: "aarch64".to_string(),
            cpu_cores: 4,
            ram_mb: 16384,
        })
    }
}

#[derive(Clone)]
struct MockGitProvider {
    commit_hash: String,
    should_fail_sync: bool,
}

impl Default for MockGitProvider {
    fn default() -> Self {
        Self {
            commit_hash: "abcdef1234567890".to_string(),
            should_fail_sync: false,
        }
    }
}

#[async_trait]
impl GitProviderPort for MockGitProvider {
    async fn sync_repository(&self, _repo: &ManagedRepository) -> Result<String, DomainError> {
        if self.should_fail_sync {
            Err(DomainError::VcsError("Failed to clone/pull repository".to_string()))
        } else {
            Ok(self.commit_hash.clone())
        }
    }

    async fn resolve_plan(&self, _repo: &ManagedRepository) -> Result<Vec<OrchestrationStep>, DomainError> {
        Ok(vec![
            OrchestrationStep::new("git_pull", "Pull latest changes"),
            OrchestrationStep::new("deploy_service", "Apply configurations"),
        ])
    }
}

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
