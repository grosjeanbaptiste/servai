use async_trait::async_trait;
use crate::domain::{
    errors::DomainError,
    repository::{ManagedRepository, RepoId},
};

#[async_trait]
pub trait RepoStorePort: Send + Sync {
    async fn save(&self, repo: ManagedRepository) -> Result<(), DomainError>;
    async fn find_by_id(&self, id: &RepoId) -> Result<Option<ManagedRepository>, DomainError>;
    async fn list(&self) -> Result<Vec<ManagedRepository>, DomainError>;
}
