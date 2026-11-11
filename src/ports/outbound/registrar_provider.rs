use async_trait::async_trait;

use crate::domain::{
    errors::DomainError,
    registrar::{DomainRegistration, NameserverConfig},
};

#[async_trait]
pub trait RegistrarProviderPort: Send + Sync {
    async fn check_availability(&self, domain: &str) -> Result<bool, DomainError>;
    async fn register_domain(&self, registration: &DomainRegistration) -> Result<(), DomainError>;
    async fn get_domain(&self, domain: &str) -> Result<Option<DomainRegistration>, DomainError>;
    async fn list_domains(&self) -> Result<Vec<DomainRegistration>, DomainError>;
    async fn set_nameservers(&self, domain: &str, ns: &NameserverConfig) -> Result<(), DomainError>;
    async fn set_auto_renew(&self, domain: &str, enable: bool) -> Result<(), DomainError>;
}
