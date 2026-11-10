use async_trait::async_trait;

use crate::domain::{
    errors::DomainError,
    stack::{MultiRepoStack, StackId},
};

#[async_trait]
pub trait StackStorePort: Send + Sync {
    async fn save(&self, stack: MultiRepoStack) -> Result<(), DomainError>;
    async fn find_by_id(&self, id: &StackId) -> Result<Option<MultiRepoStack>, DomainError>;
    async fn list(&self) -> Result<Vec<MultiRepoStack>, DomainError>;
}
