use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::domain::{
    catalog::{AdminCategory, CatalogItem},
    errors::DomainError,
    orchestration::{JobId, OrchestrationJob, RollbackPolicy},
    repository::{DriftReport, GitRef, ManagedRepository, RepoId, RepoKind},
    server::ServerId,
    stack::{MultiRepoStack, StackId, StackRepoEntry},
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

    // Catalog integration for reusing server administration repos
    async fn list_catalog(
        &self,
        category: Option<AdminCategory>,
    ) -> Result<Vec<CatalogItem>, DomainError>;
    async fn import_from_catalog(
        &self,
        catalog_id: &str,
        custom_name: Option<String>,
    ) -> Result<ManagedRepository, DomainError>;
    async fn apply_recipe(
        &self,
        server_id: &ServerId,
        catalog_id: &str,
        params: HashMap<String, String>,
    ) -> Result<OrchestrationJob, DomainError>;

    // Multi-repo composition and stack execution
    async fn create_stack(
        &self,
        name: String,
        description: String,
        entries: Vec<StackRepoEntry>,
    ) -> Result<MultiRepoStack, DomainError>;
    async fn list_stacks(&self) -> Result<Vec<MultiRepoStack>, DomainError>;
    async fn get_stack(&self, id: &StackId) -> Result<MultiRepoStack, DomainError>;
    async fn execute_stack(
        &self,
        server_id: &ServerId,
        stack_id: &StackId,
    ) -> Result<OrchestrationJob, DomainError>;
}
