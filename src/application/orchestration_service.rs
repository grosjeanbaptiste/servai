use async_trait::async_trait;
use std::sync::Arc;

use crate::domain::{
    errors::DomainError,
    orchestration::{JobId, OrchestrationJob},
    repository::{ManagedRepository, RepoId, SyncStatus},
    server::{ServerId, ServerStatus},
};
use crate::ports::{
    inbound::repo_orchestrator::{RegisterRepoCommand, RepoOrchestratorPort},
    outbound::{
        git_provider::GitProviderPort,
        job_store::JobStorePort,
        repo_store::RepoStorePort,
        server_executor::ServerExecutorPort,
        server_store::ServerStorePort,
    },
};

pub struct OrchestrationService {
    server_store: Arc<dyn ServerStorePort>,
    repo_store: Arc<dyn RepoStorePort>,
    job_store: Arc<dyn JobStorePort>,
    executor: Arc<dyn ServerExecutorPort>,
    git_provider: Arc<dyn GitProviderPort>,
}

impl OrchestrationService {
    pub fn new(
        server_store: Arc<dyn ServerStorePort>,
        repo_store: Arc<dyn RepoStorePort>,
        job_store: Arc<dyn JobStorePort>,
        executor: Arc<dyn ServerExecutorPort>,
        git_provider: Arc<dyn GitProviderPort>,
    ) -> Self {
        Self {
            server_store,
            repo_store,
            job_store,
            executor,
            git_provider,
        }
    }
}

#[async_trait]
impl RepoOrchestratorPort for OrchestrationService {
    async fn register_repo(&self, cmd: RegisterRepoCommand) -> Result<ManagedRepository, DomainError> {
        let repo = ManagedRepository::new(
            RepoId::new(),
            cmd.name,
            cmd.url,
            cmd.target_ref,
            cmd.kind,
        )?;
        self.repo_store.save(repo.clone()).await?;
        Ok(repo)
    }

    async fn get_repo(&self, id: &RepoId) -> Result<ManagedRepository, DomainError> {
        self.repo_store
            .find_by_id(id)
            .await?
            .ok_or_else(|| DomainError::NotFound {
                entity: "ManagedRepository",
                id: id.to_string(),
            })
    }

    async fn list_repos(&self) -> Result<Vec<ManagedRepository>, DomainError> {
        self.repo_store.list().await
    }

    async fn sync_repo(&self, id: &RepoId) -> Result<ManagedRepository, DomainError> {
        let mut repo = self.get_repo(id).await?;
        repo.set_sync_status(SyncStatus::Syncing);
        self.repo_store.save(repo.clone()).await?;

        match self.git_provider.sync_repository(&repo).await {
            Ok(commit_hash) => {
                repo.set_sync_status(SyncStatus::Synced);
                repo.set_last_commit_hash(commit_hash);
                self.repo_store.save(repo.clone()).await?;
                Ok(repo)
            }
            Err(e) => {
                repo.set_sync_status(SyncStatus::Failed);
                self.repo_store.save(repo).await?;
                Err(e)
            }
        }
    }

    async fn trigger_orchestration(
        &self,
        server_id: &ServerId,
        repo_id: &RepoId,
    ) -> Result<OrchestrationJob, DomainError> {
        let server = self
            .server_store
            .find_by_id(server_id)
            .await?
            .ok_or_else(|| DomainError::NotFound {
                entity: "Server",
                id: server_id.to_string(),
            })?;

        if server.status() != ServerStatus::Online {
            return Err(DomainError::Conflict(format!(
                "Cannot orchestrate on server '{}' with status {:?}",
                server.name().as_str(),
                server.status()
            )));
        }

        let repo = self.get_repo(repo_id).await?;
        let steps = self.git_provider.resolve_plan(&repo).await?;

        let mut job = OrchestrationJob::new(JobId::new(), *server_id, *repo_id, steps)?;
        job.start()?;
        self.job_store.save(job.clone()).await?;

        // Execute steps sequentially
        let step_count = job.steps().len();
        for i in 0..step_count {
            let step_name = job.steps()[i].step.name.clone();
            let cmd_to_run = format!("servai-run-step:{}", step_name);

            match self.executor.execute_command(&server, &cmd_to_run).await {
                Ok(res) => {
                    if res.exit_code == 0 {
                        job.record_step_success(i, res.stdout)?;
                    } else {
                        job.record_step_failure(
                            i,
                            format!("Step failed with exit code {}", res.exit_code),
                            res.stderr,
                        )?;
                        break;
                    }
                }
                Err(err) => {
                    job.record_step_failure(i, "Step execution error", err.to_string())?;
                    break;
                }
            }
        }

        self.job_store.save(job.clone()).await?;
        Ok(job)
    }

    async fn get_job(&self, id: &JobId) -> Result<OrchestrationJob, DomainError> {
        self.job_store
            .find_by_id(id)
            .await?
            .ok_or_else(|| DomainError::NotFound {
                entity: "OrchestrationJob",
                id: id.to_string(),
            })
    }
}
