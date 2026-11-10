use async_trait::async_trait;
use crate::domain::{
    errors::DomainError,
    orchestration::OrchestrationStep,
    repository::ManagedRepository,
};

#[async_trait]
pub trait GitProviderPort: Send + Sync {
    async fn sync_repository(&self, repo: &ManagedRepository) -> Result<String, DomainError>;
    async fn resolve_plan(&self, repo: &ManagedRepository) -> Result<Vec<OrchestrationStep>, DomainError>;
}
