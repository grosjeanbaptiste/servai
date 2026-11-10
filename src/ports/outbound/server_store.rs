use async_trait::async_trait;
use crate::domain::{
    errors::DomainError,
    server::{Server, ServerId},
};

#[async_trait]
pub trait ServerStorePort: Send + Sync {
    async fn save(&self, server: Server) -> Result<(), DomainError>;
    async fn find_by_id(&self, id: &ServerId) -> Result<Option<Server>, DomainError>;
    async fn list(&self) -> Result<Vec<Server>, DomainError>;
    async fn delete(&self, id: &ServerId) -> Result<(), DomainError>;
}
