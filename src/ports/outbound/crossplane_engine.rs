use async_trait::async_trait;
use std::path::Path;

use crate::domain::{
    engines::crossplane::{
        CrossplaneClaim, CrossplaneProviderStatus, CrossplaneReconcileResult, CrossplaneResource,
    },
    errors::DomainError,
};

#[async_trait]
pub trait CrossplaneEnginePort: Send + Sync {
    async fn apply_claim(&self, claim: &CrossplaneClaim) -> Result<CrossplaneReconcileResult, DomainError>;
    async fn get_claim_status(&self, namespace: &str, name: &str) -> Result<CrossplaneReconcileResult, DomainError>;
    async fn delete_claim(&self, namespace: &str, name: &str) -> Result<(), DomainError>;
    async fn list_managed_resources(&self, namespace: Option<&str>) -> Result<Vec<CrossplaneResource>, DomainError>;
    async fn list_providers(&self) -> Result<Vec<CrossplaneProviderStatus>, DomainError>;
    async fn render_composition(
        &self,
        claim_path: &Path,
        composition_path: &Path,
    ) -> Result<Vec<CrossplaneResource>, DomainError>;
}
