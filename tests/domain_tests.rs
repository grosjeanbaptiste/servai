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

#[test]
fn test_server_concurrency_lock() {
    let mut server = Server::new(
        ServerId::new(),
        ServerName::new("prod-app-01").unwrap(),
        HostAddress::new("10.0.0.1", 22).unwrap(),
        SshCredentials::Agent,
        None,
    );

    let job_1 = JobId::new();
    let job_2 = JobId::new();

    assert!(!server.is_locked());
    assert!(server.acquire_lock(job_1).is_ok());
    assert!(server.is_locked());
    assert_eq!(server.locked_by_job(), Some(job_1));

    // Second lock attempt must fail with Conflict
    let err = server.acquire_lock(job_2).unwrap_err();
    assert!(matches!(err, DomainError::Conflict(_)));

    // Release with wrong job id must fail
    assert!(matches!(
        server.release_lock(job_2),
        Err(DomainError::Conflict(_))
    ));

    // Release with right job id succeeds
    assert!(server.release_lock(job_1).is_ok());
    assert!(!server.is_locked());
}

#[test]
fn test_server_metrics_validation() {
    use servai::domain::server::ServerMetrics;

    let metrics = ServerMetrics::new(
        42.5,   // CPU %
        8192,   // RAM used MB
        16384,  // RAM total MB
        120.0,  // Disk used GB
        500.0,  // Disk total GB
        360000, // Uptime seconds
        [0.45, 0.52, 0.60], // Load average
    )
    .expect("valid metrics");

    assert_eq!(metrics.cpu_usage_pct, 42.5);
    assert_eq!(metrics.ram_percentage(), 50.0);
    assert_eq!(metrics.disk_percentage(), 24.0);

    // Invalid CPU > 100%
    assert!(matches!(
        ServerMetrics::new(105.0, 100, 200, 10.0, 50.0, 1000, [0.1, 0.1, 0.1]),
        Err(DomainError::ValidationError(_))
    ));

    // Invalid RAM used > total
    assert!(matches!(
        ServerMetrics::new(50.0, 300, 200, 10.0, 50.0, 1000, [0.1, 0.1, 0.1]),
        Err(DomainError::ValidationError(_))
    ));
}

#[test]
fn test_service_action_and_status() {
    use servai::domain::server::{ServiceAction, ServiceStatus};

    assert_eq!(ServiceAction::Restart.as_systemd_command(), "restart");
    assert_eq!(ServiceAction::Start.as_systemd_command(), "start");
    assert_eq!(ServiceAction::Stop.as_systemd_command(), "stop");

    assert!(ServiceStatus::Active.is_healthy());
    assert!(!ServiceStatus::Failed.is_healthy());
    assert!(!ServiceStatus::Inactive.is_healthy());
}

#[test]
fn test_orchestration_job_cancellation() {
    let job_id = JobId::new();
    let steps = vec![
        OrchestrationStep::new("step_1", "Download artifacts"),
        OrchestrationStep::new("step_2", "Deploy services"),
    ];

    let mut job = OrchestrationJob::new(job_id, ServerId::new(), RepoId::new(), steps).unwrap();

    // Cannot cancel pending job before starting (or cancels to Cancelled)
    assert!(job.cancel("User aborted").is_ok());
    assert_eq!(job.status(), ExecutionStatus::Cancelled);

    // Cannot start a cancelled job
    assert!(matches!(
        job.start(),
        Err(DomainError::InvalidStateTransition { .. })
    ));
}

#[test]
fn test_orchestration_job_with_rollback() {
    use servai::domain::orchestration::RollbackPolicy;

    let job_id = JobId::new();
    let mut step1 = OrchestrationStep::new("setup_db", "Prepare database schema");
    step1.rollback_command = Some("drop database servai_temp".to_string());

    let mut step2 = OrchestrationStep::new("migrate_data", "Migrate users");
    step2.rollback_command = Some("truncate table users".to_string());

    let mut job = OrchestrationJob::new_with_policy(
        job_id,
        ServerId::new(),
        RepoId::new(),
        vec![step1, step2],
        RollbackPolicy::RollbackOnFailure,
    )
    .unwrap();

    job.start().unwrap();
    job.record_step_success(0, "DB schema prepared").unwrap();

    // Step 1 fails
    job.record_step_failure(1, "Migration syntax error", "Column missing")
        .unwrap();

    assert_eq!(job.status(), ExecutionStatus::Failed);

    // Rollback execution
    let rollback_steps = job.compensating_rollback_steps();
    assert_eq!(rollback_steps.len(), 1); // Only step 0 completed and needs rollback
    assert_eq!(
        rollback_steps[0].rollback_command.as_deref(),
        Some("drop database servai_temp")
    );

    job.record_rollback_completed("Compensating rollback finished successfully")
        .unwrap();
    assert_eq!(job.status(), ExecutionStatus::RolledBack);
}

#[test]
fn test_drift_detection_report() {
    use servai::domain::repository::DriftReport;

    let report = DriftReport::new(
        vec!["/etc/nginx/nginx.conf".to_string()],
        vec!["/etc/nginx/conf.d/unexpected.conf".to_string()],
        vec![],
    );

    assert!(report.has_drift());
    assert_eq!(report.modified_files.len(), 1);
    assert_eq!(report.untracked_files.len(), 1);

    let clean_report = DriftReport::new(vec![], vec![], vec![]);
    assert!(!clean_report.has_drift());
}

