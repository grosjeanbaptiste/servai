use async_trait::async_trait;

use crate::domain::{
    errors::DomainError,
    registrar::{DomainRegistration, NameserverConfig},
};

#[async_trait]
pub trait RegistrarAdminPort: Send + Sync {
    async fn check_domain_availability(&self, domain: &str) -> Result<bool, DomainError>;
    async fn register_domain(&self, registration: DomainRegistration) -> Result<DomainRegistration, DomainError>;
    async fn get_domain(&self, domain: &str) -> Result<DomainRegistration, DomainError>;
    async fn list_domains(&self) -> Result<Vec<DomainRegistration>, DomainError>;
    async fn update_domain_nameservers(&self, domain: &str, ns: NameserverConfig) -> Result<(), DomainError>;
    async fn set_domain_auto_renew(&self, domain: &str, enable: bool) -> Result<(), DomainError>;
}
