use async_trait::async_trait;
use crate::domain::{
    errors::DomainError,
    orchestration::{JobId, OrchestrationJob},
    server::ServerId,
};

#[async_trait]
pub trait JobStorePort: Send + Sync {
    async fn save(&self, job: OrchestrationJob) -> Result<(), DomainError>;
    async fn find_by_id(&self, id: &JobId) -> Result<Option<OrchestrationJob>, DomainError>;
    async fn list_by_server(&self, server_id: &ServerId) -> Result<Vec<OrchestrationJob>, DomainError>;
    async fn list_all(&self) -> Result<Vec<OrchestrationJob>, DomainError>;
}
