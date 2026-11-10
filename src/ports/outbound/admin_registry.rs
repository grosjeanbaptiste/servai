use async_trait::async_trait;

use crate::domain::{
    errors::DomainError,
    registry::{AdminPackage, RegistryQuery, ServerProfilePreset},
};

#[async_trait]
pub trait AdminRegistryPort: Send + Sync {
    async fn search(&self, query: &RegistryQuery) -> Result<Vec<AdminPackage>, DomainError>;
    async fn get_package(&self, id: &str) -> Result<Option<AdminPackage>, DomainError>;
    async fn list_presets(&self) -> Result<Vec<ServerProfilePreset>, DomainError>;
    async fn resolve_dependencies(&self, package_id: &str) -> Result<Vec<AdminPackage>, DomainError>;
}
