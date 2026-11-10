use async_trait::async_trait;
use std::sync::Arc;

use crate::domain::{
    errors::DomainError,
    orchestration::{JobId, OrchestrationJob, RollbackPolicy},
    repository::{DriftReport, ManagedRepository, RepoId, SyncStatus},
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
        self.trigger_orchestration_with_options(server_id, repo_id, RollbackPolicy::Halt)
            .await
    }

    async fn trigger_orchestration_with_options(
        &self,
        server_id: &ServerId,
        repo_id: &RepoId,
        rollback_policy: RollbackPolicy,
    ) -> Result<OrchestrationJob, DomainError> {
        let mut server = self
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

        let mut job = OrchestrationJob::new_with_policy(
            JobId::new(),
            *server_id,
            *repo_id,
            steps,
            rollback_policy,
        )?;

        // Acquire server concurrency lock
        server.acquire_lock(job.id())?;
        self.server_store.save(server.clone()).await?;

        job.start()?;
        self.job_store.save(job.clone()).await?;

        // Execute steps sequentially
        let step_count = job.steps().len();
        let mut step_failed = false;

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
                        step_failed = true;
                        break;
                    }
                }
                Err(err) => {
                    job.record_step_failure(i, "Step execution error", err.to_string())?;
                    step_failed = true;
                    break;
                }
            }
        }

        // Check if rollback needed
        if step_failed && rollback_policy == RollbackPolicy::RollbackOnFailure {
            let rollback_steps = job.compensating_rollback_steps();
            for rb_step in rollback_steps {
                if let Some(rb_cmd) = rb_step.rollback_command {
                    let _ = self.executor.execute_command(&server, &rb_cmd).await;
                }
            }
            job.record_rollback_completed("Compensating rollback finished")?;
        }

        // Release server lock
        if let Ok(Some(mut fresh_server)) = self.server_store.find_by_id(server_id).await {
            let _ = fresh_server.release_lock(job.id());
            let _ = self.server_store.save(fresh_server).await;
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

    async fn cancel_job(&self, id: &JobId, reason: &str) -> Result<OrchestrationJob, DomainError> {
        let mut job = self.get_job(id).await?;
        job.cancel(reason)?;

        // Ensure server lock is released on cancel
        if let Ok(Some(mut server)) = self.server_store.find_by_id(&job.server_id()).await {
            let _ = server.release_lock(job.id());
            let _ = self.server_store.save(server).await;
        }

        self.job_store.save(job.clone()).await?;
        Ok(job)
    }

    async fn detect_drift(
        &self,
        server_id: &ServerId,
        repo_id: &RepoId,
    ) -> Result<DriftReport, DomainError> {
        let _server = self.server_store.find_by_id(server_id).await?.ok_or_else(|| {
            DomainError::NotFound {
                entity: "Server",
                id: server_id.to_string(),
            }
        })?;
        let repo = self.get_repo(repo_id).await?;

        self.git_provider.detect_drift(server_id, &repo).await
    }
}
