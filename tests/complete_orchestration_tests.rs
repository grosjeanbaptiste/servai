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
        orchestration::ExecutionStatus,
        repository::{GitRef, ManagedRepository, RepoId, RepoKind},
        server::{ServerId, SshCredentials},
        workflow::{
            DesiredServerState, FleetRolloutCommand, FleetRolloutStrategy,
            ScheduledJobCommand, WorkflowAction, WorkflowStep,
        },
    },
    ports::{
        inbound::{
            repo_orchestrator::RepoOrchestratorPort,
            server_admin::ServerAdminPort,
        },
        outbound::repo_store::RepoStorePort,
    },
};

async fn setup_test_orchestrator() -> (
    Arc<OrchestrationService>,
    Arc<ServerService>,
    ServerId,
    ServerId,
) {
    let server_store = Arc::new(InMemoryServerStore::new());
    let repo_store = Arc::new(InMemoryRepoStore::new());
    let job_store = Arc::new(InMemoryJobStore::new());
    let executor = Arc::new(MockServerExecutor::default());
    let git_provider = Arc::new(MockGitProvider::default());

    let server_service = Arc::new(ServerService::new(server_store.clone(), executor.clone()));
    let orchestrator = Arc::new(OrchestrationService::new(
        server_store.clone(),
        repo_store.clone(),
        job_store.clone(),
        executor.clone(),
        git_provider.clone(),
    ));

    // Register two test servers
    let s1 = server_service
        .register_server(RegisterServerCommand {
            name: "srv-node-01".to_string(),
            ip: "192.168.1.101".to_string(),
            port: 22,
            credentials: SshCredentials::Agent,
            tags: vec!["web".to_string(), "prod".to_string()],
        })
        .await
        .unwrap();

    let s2 = server_service
        .register_server(RegisterServerCommand {
            name: "srv-node-02".to_string(),
            ip: "192.168.1.102".to_string(),
            port: 22,
            credentials: SshCredentials::Agent,
            tags: vec!["web".to_string(), "prod".to_string()],
        })
        .await
        .unwrap();

    // Bring both servers online via healthcheck ping
    server_service.health_check(&s1.id()).await.unwrap();
    server_service.health_check(&s2.id()).await.unwrap();

    // Register a test repo
    let test_repo = ManagedRepository::new(
        RepoId::new(),
        "hardening-repo".to_string(),
        "https://github.com/dev-sec/ansible-collection-hardening.git".to_string(),
        GitRef::Branch("master".to_string()),
        RepoKind::Playbook,
    )
    .unwrap();
    repo_store.save(test_repo).await.unwrap();

    (orchestrator, server_service, s1.id(), s2.id())
}

#[tokio::test]
async fn test_orchestration_list_and_detailed_logs() {
    let (orchestrator, _, s1, _) = setup_test_orchestrator().await;
    let repos = orchestrator.list_repos().await.unwrap();
    let repo_id = repos[0].id();

    // Trigger two jobs
    let job1 = orchestrator.trigger_orchestration(&s1, &repo_id).await.unwrap();
    let _job2 = orchestrator.trigger_orchestration(&s1, &repo_id).await.unwrap();

    // 1. List all jobs
    let all_jobs = orchestrator.list_jobs(None, None).await.unwrap();
    assert!(all_jobs.len() >= 2);

    // 2. Filter by server
    let s1_jobs = orchestrator.list_jobs(Some(&s1), None).await.unwrap();
    assert_eq!(s1_jobs.len(), 2);

    // 3. Get detailed job logs
    let logs = orchestrator.get_job_logs(&job1.id()).await.unwrap();
    assert_eq!(logs.job_id, job1.id());
    assert!(!logs.steps.is_empty());
    assert!(logs.steps[0].status == ExecutionStatus::Succeeded);
}

#[tokio::test]
async fn test_custom_workflow_pipeline_definition_and_execution() {
    let (orchestrator, _, s1, _) = setup_test_orchestrator().await;

    // Define a 4-step workflow
    let steps = vec![
        WorkflowStep::new("preflight", "Vérification espace disque", WorkflowAction::ShellCommand {
            command: "df -h /".to_string(),
        }),
        WorkflowStep::new("sync-configs", "Synchronisation des dotfiles", WorkflowAction::ShellCommand {
            command: "stow -v -R starship zsh git".to_string(),
        }),
        WorkflowStep::new("dns-announce", "Publication de l'adresse IP", WorkflowAction::DnsRecordUpsert {
            domain: "bioxus.cloud".to_string(),
            record_type: "A".to_string(),
            value: "192.168.1.101".to_string(),
        }),
        WorkflowStep::new("health-probe", "Vérification service web", WorkflowAction::HttpHealthCheck {
            url: "http://192.168.1.101/health".to_string(),
            expected_status: 200,
        }),
    ];

    let wf = orchestrator
        .create_workflow("node-provisioning", "Pipeline de provisionnement complet", steps)
        .await
        .unwrap();

    assert_eq!(wf.name, "node-provisioning");
    assert_eq!(wf.steps.len(), 4);

    // Execute the custom workflow
    let job = orchestrator.execute_workflow(&s1, &wf.id).await.unwrap();
    assert_eq!(job.status(), ExecutionStatus::Succeeded);
    assert_eq!(job.steps().len(), 4);
    assert_eq!(job.steps()[0].step.name, "preflight");
    assert_eq!(job.steps()[3].step.name, "health-probe");
}

#[tokio::test]
async fn test_fleet_orchestration_across_server_tags() {
    let (orchestrator, _, s1, s2) = setup_test_orchestrator().await;

    // Fleet rollout command targeting "web" tag
    let fleet_cmd = FleetRolloutCommand {
        target_tag: Some("web".to_string()),
        server_ids: vec![],
        strategy: FleetRolloutStrategy::Parallel,
        action: WorkflowAction::ShellCommand {
            command: "uptime".to_string(),
        },
    };

    let fleet_res = orchestrator.execute_fleet_rollout(fleet_cmd).await.unwrap();
    assert_eq!(fleet_res.total_targets, 2);
    assert_eq!(fleet_res.succeeded_count, 2);
    assert_eq!(fleet_res.failed_count, 0);
    assert_eq!(fleet_res.jobs.len(), 2);
    assert!(fleet_res.jobs.iter().any(|j| j.server_id() == s1));
    assert!(fleet_res.jobs.iter().any(|j| j.server_id() == s2));
}

#[tokio::test]
async fn test_scheduled_orchestration_lifecycle_and_manual_trigger() {
    let (orchestrator, _, s1, _) = setup_test_orchestrator().await;

    // Create a scheduled job (nightly backup)
    let sched_cmd = ScheduledJobCommand {
        name: "nightly-borg-backup".to_string(),
        cron_expression: "0 2 * * *".to_string(),
        server_id: s1,
        action: WorkflowAction::ShellCommand {
            command: "borg create ::backup-{now}".to_string(),
        },
    };

    let schedule = orchestrator.create_schedule(sched_cmd).await.unwrap();
    assert_eq!(schedule.name, "nightly-borg-backup");
    assert_eq!(schedule.cron_expression, "0 2 * * *");
    assert!(schedule.enabled);

    // List schedules
    let list = orchestrator.list_schedules().await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, schedule.id);

    // Trigger schedule manually
    let job = orchestrator.run_schedule_now(&schedule.id).await.unwrap();
    assert_eq!(job.server_id(), s1);
    assert_eq!(job.status(), ExecutionStatus::Succeeded);

    // Delete schedule
    orchestrator.delete_schedule(&schedule.id).await.unwrap();
    let empty_list = orchestrator.list_schedules().await.unwrap();
    assert!(empty_list.is_empty());
}

#[tokio::test]
async fn test_declarative_desired_state_reconciliation() {
    let (orchestrator, _, s1, _) = setup_test_orchestrator().await;

    // Define desired state for s1
    let desired = DesiredServerState {
        server_id: s1,
        required_packages: vec!["dev-sec-hardening".to_string(), "geerlingguy-docker".to_string()],
        bound_domain: Some("app.bioxus.cloud".to_string()),
        wireguard_enabled: true,
    };

    let recon_result = orchestrator.reconcile_server_state(desired).await.unwrap();
    assert_eq!(recon_result.server_id, s1);
    assert!(recon_result.drift_detected);
    assert_eq!(recon_result.remediation_jobs.len(), 4); // Hardening + Docker + Domain + WireGuard
    assert!(recon_result.remediation_jobs.iter().all(|j| j.status() == ExecutionStatus::Succeeded));
}
