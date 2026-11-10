use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::domain::{
    errors::DomainError,
    orchestration::{JobId, OrchestrationJob, RollbackPolicy},
    repository::{DriftReport, GitRef, ManagedRepository, RepoId, RepoKind},
    server::ServerId,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRepoCommand {
    pub name: String,
    pub url: String,
    pub target_ref: GitRef,
    pub kind: RepoKind,
}

#[async_trait]
pub trait RepoOrchestratorPort: Send + Sync {
    async fn register_repo(&self, cmd: RegisterRepoCommand) -> Result<ManagedRepository, DomainError>;
    async fn get_repo(&self, id: &RepoId) -> Result<ManagedRepository, DomainError>;
    async fn list_repos(&self) -> Result<Vec<ManagedRepository>, DomainError>;
    async fn sync_repo(&self, id: &RepoId) -> Result<ManagedRepository, DomainError>;
    async fn trigger_orchestration(
        &self,
        server_id: &ServerId,
        repo_id: &RepoId,
    ) -> Result<OrchestrationJob, DomainError>;
    async fn trigger_orchestration_with_options(
        &self,
        server_id: &ServerId,
        repo_id: &RepoId,
        rollback_policy: RollbackPolicy,
    ) -> Result<OrchestrationJob, DomainError>;
    async fn get_job(&self, id: &JobId) -> Result<OrchestrationJob, DomainError>;
    async fn cancel_job(&self, id: &JobId, reason: &str) -> Result<OrchestrationJob, DomainError>;
    async fn detect_drift(
        &self,
        server_id: &ServerId,
        repo_id: &RepoId,
    ) -> Result<DriftReport, DomainError>;
}
