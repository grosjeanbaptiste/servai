use async_trait::async_trait;

use crate::domain::{
    catalog::CatalogItem,
    errors::DomainError,
};

#[async_trait]
pub trait RepoCatalogPort: Send + Sync {
    async fn list_catalog(&self) -> Result<Vec<CatalogItem>, DomainError>;
    async fn find_catalog_item(&self, id: &str) -> Result<Option<CatalogItem>, DomainError>;
}
