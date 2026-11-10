use servai::domain::{
    errors::DomainError,
    orchestration::{ExecutionStatus, JobId, OrchestrationJob, OrchestrationStep},
    repository::{GitRef, ManagedRepository, RepoId, RepoKind, SyncStatus},
    server::{HostAddress, Server, ServerId, ServerName, ServerSpec, ServerStatus, SshCredentials},
};

#[test]
fn test_server_creation_nominal() {
    let id = ServerId::new();
    let name = ServerName::new("prod-web-01").expect("valid server name");
    let host = HostAddress::new("192.168.1.100", 22).expect("valid host");
    let creds = SshCredentials::PrivateKey {
        username: "ubuntu".to_string(),
        key_content: "-----BEGIN OPENSSH PRIVATE KEY-----\ntest\n-----END OPENSSH PRIVATE KEY-----".to_string(),
        passphrase: None,
    };
    let spec = ServerSpec {
        os: "Ubuntu 24.04 LTS".to_string(),
        arch: "x86_64".to_string(),
        cpu_cores: 8,
        ram_mb: 32768,
    };

    let server = Server::new(id, name.clone(), host.clone(), creds, Some(spec));

    assert_eq!(server.id(), id);
    assert_eq!(server.name().as_str(), "prod-web-01");
    assert_eq!(server.host().ip(), "192.168.1.100");
    assert_eq!(server.host().port(), 22);
    assert_eq!(server.status(), ServerStatus::Pending);
}

#[test]
fn test_server_validation_edge_cases() {
    // Empty name
    assert!(matches!(
        ServerName::new(""),
        Err(DomainError::ValidationError(_))
    ));

    // Name too long (> 64 chars)
    let long_name = "a".repeat(65);
    assert!(matches!(
        ServerName::new(&long_name),
        Err(DomainError::ValidationError(_))
    ));

    // Name with invalid characters (spaces, special chars)
    assert!(matches!(
        ServerName::new("prod web 01"),
        Err(DomainError::ValidationError(_))
    ));

    // Invalid port 0
    assert!(matches!(
        HostAddress::new("10.0.0.1", 0),
        Err(DomainError::ValidationError(_))
    ));

    // Invalid IP/hostname
    assert!(matches!(
        HostAddress::new("not an ip or host name!", 22),
        Err(DomainError::ValidationError(_))
    ));
}

#[test]
fn test_server_lifecycle_state_machine() {
    let mut server = Server::new(
        ServerId::new(),
        ServerName::new("db-master").unwrap(),
        HostAddress::new("db.internal", 2222).unwrap(),
        SshCredentials::Agent,
        None,
    );

    assert_eq!(server.status(), ServerStatus::Pending);

    // Pending -> Online
    assert!(server.transition_to(ServerStatus::Online).is_ok());
    assert_eq!(server.status(), ServerStatus::Online);

    // Online -> Maintenance
    assert!(server.transition_to(ServerStatus::Maintenance).is_ok());
    assert_eq!(server.status(), ServerStatus::Maintenance);

    // Maintenance -> Online
    assert!(server.transition_to(ServerStatus::Online).is_ok());

    // Online -> Degraded
    assert!(server.transition_to(ServerStatus::Degraded).is_ok());

    // Degraded -> Offline
    assert!(server.transition_to(ServerStatus::Offline).is_ok());

    // Offline -> Online
    assert!(server.transition_to(ServerStatus::Online).is_ok());
}

#[test]
fn test_server_invalid_state_transitions() {
    let mut server = Server::new(
        ServerId::new(),
        ServerName::new("app-worker").unwrap(),
        HostAddress::new("worker.internal", 22).unwrap(),
        SshCredentials::Agent,
        None,
    );

    // Pending cannot jump directly to Degraded
    let err = server.transition_to(ServerStatus::Degraded).unwrap_err();
    assert!(matches!(err, DomainError::InvalidStateTransition { .. }));
}

#[test]
fn test_managed_repository_creation_and_sync_policy() {
    let id = RepoId::new();
    let repo = ManagedRepository::new(
        id,
        "infrastructure-configs".to_string(),
        "git@github.com:myorg/infra-configs.git".to_string(),
        GitRef::Branch("main".to_string()),
        RepoKind::Playbook,
    )
    .expect("valid repo");

    assert_eq!(repo.id(), id);
    assert_eq!(repo.name(), "infrastructure-configs");
    assert_eq!(repo.sync_status(), SyncStatus::Pending);

    // Invalid git url
    assert!(matches!(
        ManagedRepository::new(
            RepoId::new(),
            "invalid".to_string(),
            "not_a_valid_url".to_string(),
            GitRef::Branch("main".to_string()),
            RepoKind::DockerCompose
        ),
        Err(DomainError::ValidationError(_))
    ));
}

#[test]
fn test_orchestration_job_state_machine_and_execution() {
    let job_id = JobId::new();
    let server_id = ServerId::new();
    let repo_id = RepoId::new();

    let steps = vec![
        OrchestrationStep::new("git_pull", "Fetch and checkout target commit"),
        OrchestrationStep::new("syntax_check", "Validate playbook/manifest syntax"),
        OrchestrationStep::new("apply_changes", "Apply configuration to target server"),
    ];

    let mut job = OrchestrationJob::new(job_id, server_id, repo_id, steps).expect("valid job");

    assert_eq!(job.status(), ExecutionStatus::Pending);
    assert_eq!(job.current_step_index(), 0);

    // Start job
    job.start().expect("job started");
    assert_eq!(job.status(), ExecutionStatus::Running);

    // Step 0 success
    job.record_step_success(0, "Git pull succeeded: commit abc1234")
        .expect("step 0 recorded");
    assert_eq!(job.current_step_index(), 1);

    // Step 1 success
    job.record_step_success(1, "Syntax valid").expect("step 1 recorded");
    assert_eq!(job.current_step_index(), 2);

    // Step 2 success
    job.record_step_success(2, "Changes applied successfully")
        .expect("step 2 recorded");

    // All steps done -> Job Succeeded
    assert_eq!(job.status(), ExecutionStatus::Succeeded);
}

#[test]
fn test_orchestration_job_step_failure() {
    let job_id = JobId::new();
    let server_id = ServerId::new();
    let repo_id = RepoId::new();

    let steps = vec![
        OrchestrationStep::new("step_1", "Initial setup"),
        OrchestrationStep::new("step_2", "Service deployment"),
    ];

    let mut job = OrchestrationJob::new(job_id, server_id, repo_id, steps).unwrap();
    job.start().unwrap();

    // Step 0 fails
    job.record_step_failure(0, "Command failed: exit code 1", "Port 80 already in use")
        .expect("step failure recorded");

    assert_eq!(job.status(), ExecutionStatus::Failed);
    assert!(job.error_details().is_some());
}
