use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::domain::{
    errors::DomainError,
    registrar::{ContactInfo, DomainRegistration, DomainStatus, NameserverConfig, RegistrarProvider},
};
use crate::ports::outbound::registrar_provider::RegistrarProviderPort;

#[derive(Clone)]
pub struct MockRegistrarProvider {
    pub should_fail: bool,
    domains: Arc<RwLock<HashMap<String, DomainRegistration>>>,
}

impl Default for MockRegistrarProvider {
    fn default() -> Self {
        let mut map = HashMap::new();
        let contact = ContactInfo::new("Admin", "admin@servai.net", "+1.5550100", "Paris", "FR").unwrap();
        let ns = NameserverConfig::new(vec!["ns1.servai.net".to_string(), "ns2.servai.net".to_string()]).unwrap();
        let reg = DomainRegistration::new(
            "servai.net",
            RegistrarProvider::Cloudflare,
            DomainStatus::Active,
            contact,
            ns,
            true,
        )
        .unwrap();
        map.insert("servai.net".to_string(), reg);

        Self {
            should_fail: false,
            domains: Arc::new(RwLock::new(map)),
        }
    }
}

#[async_trait]
impl RegistrarProviderPort for MockRegistrarProvider {
    async fn check_availability(&self, domain: &str) -> Result<bool, DomainError> {
        if self.should_fail {
            return Err(DomainError::ExecutionFailed {
                reason: "Registrar API rate limit exceeded".to_string(),
                details: None,
            });
        }
        let exists = self.domains.read().unwrap().contains_key(domain);
        Ok(!exists)
    }

    async fn register_domain(&self, registration: &DomainRegistration) -> Result<(), DomainError> {
        if self.should_fail {
            return Err(DomainError::ExecutionFailed {
                reason: format!("Failed to register domain {}", registration.domain()),
                details: None,
            });
        }
        self.domains
            .write()
            .unwrap()
            .insert(registration.domain().to_string(), registration.clone());
        Ok(())
    }

    async fn get_domain(&self, domain: &str) -> Result<Option<DomainRegistration>, DomainError> {
        Ok(self.domains.read().unwrap().get(domain).cloned())
    }

    async fn list_domains(&self) -> Result<Vec<DomainRegistration>, DomainError> {
        Ok(self.domains.read().unwrap().values().cloned().collect())
    }

    async fn set_nameservers(&self, domain: &str, ns: &NameserverConfig) -> Result<(), DomainError> {
        if self.should_fail {
            return Err(DomainError::ExecutionFailed {
                reason: format!("Failed to update nameservers for {}", domain),
                details: None,
            });
        }
        let mut map = self.domains.write().unwrap();
        let reg = map.get_mut(domain).ok_or_else(|| DomainError::NotFound {
            entity: "DomainRegistration",
            id: domain.to_string(),
        })?;
        reg.set_nameservers(ns.clone());
        Ok(())
    }

    async fn set_auto_renew(&self, domain: &str, enable: bool) -> Result<(), DomainError> {
        if self.should_fail {
            return Err(DomainError::ExecutionFailed {
                reason: format!("Failed to update auto-renew for {}", domain),
                details: None,
            });
        }
        let mut map = self.domains.write().unwrap();
        let reg = map.get_mut(domain).ok_or_else(|| DomainError::NotFound {
            entity: "DomainRegistration",
            id: domain.to_string(),
        })?;
        reg.set_auto_renew(enable);
        Ok(())
    }
}
