use async_trait::async_trait;
use std::sync::Arc;

use crate::domain::{
    errors::DomainError,
    registrar::{DomainRegistration, NameserverConfig},
};
use crate::ports::{
    inbound::registrar_admin::RegistrarAdminPort,
    outbound::registrar_provider::RegistrarProviderPort,
};

pub struct RegistrarService {
    registrar_provider: Arc<dyn RegistrarProviderPort>,
}

impl RegistrarService {
    pub fn new(registrar_provider: Arc<dyn RegistrarProviderPort>) -> Self {
        Self { registrar_provider }
    }
}

#[async_trait]
impl RegistrarAdminPort for RegistrarService {
    async fn check_domain_availability(&self, domain: &str) -> Result<bool, DomainError> {
        self.registrar_provider.check_availability(domain).await
    }

    async fn register_domain(
        &self,
        registration: DomainRegistration,
    ) -> Result<DomainRegistration, DomainError> {
        self.registrar_provider.register_domain(&registration).await?;
        Ok(registration)
    }

    async fn get_domain(&self, domain: &str) -> Result<DomainRegistration, DomainError> {
        self.registrar_provider
            .get_domain(domain)
            .await?
            .ok_or_else(|| DomainError::NotFound {
                entity: "DomainRegistration",
                id: domain.to_string(),
            })
    }

    async fn list_domains(&self) -> Result<Vec<DomainRegistration>, DomainError> {
        self.registrar_provider.list_domains().await
    }

    async fn update_domain_nameservers(
        &self,
        domain: &str,
        ns: NameserverConfig,
    ) -> Result<(), DomainError> {
        self.registrar_provider.set_nameservers(domain, &ns).await
    }

    async fn set_domain_auto_renew(&self, domain: &str, enable: bool) -> Result<(), DomainError> {
        self.registrar_provider.set_auto_renew(domain, enable).await
    }
}
