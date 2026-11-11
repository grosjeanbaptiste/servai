use serde::{Deserialize, Serialize};

use crate::domain::errors::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegistrarProvider {
    Cloudflare,
    Ovh,
    Gandi,
    Namecheap,
    Route53,
    Porkbun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DomainStatus {
    Active,
    Pending,
    Expired,
    Suspended,
    Transferring,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContactInfo {
    pub name: String,
    pub email: String,
    pub phone: String,
    pub city: String,
    pub country: String,
}

impl ContactInfo {
    pub fn new(
        name: impl Into<String>,
        email: impl Into<String>,
        phone: impl Into<String>,
        city: impl Into<String>,
        country: impl Into<String>,
    ) -> Result<Self, DomainError> {
        let n = name.into();
        let e = email.into();
        if n.trim().is_empty() {
            return Err(DomainError::ValidationError("Contact name cannot be empty".to_string()));
        }
        if !e.contains('@') {
            return Err(DomainError::ValidationError("Invalid email address for domain contact".to_string()));
        }

        Ok(Self {
            name: n.trim().to_string(),
            email: e.trim().to_string(),
            phone: phone.into().trim().to_string(),
            city: city.into().trim().to_string(),
            country: country.into().trim().to_string(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NameserverConfig {
    servers: Vec<String>,
}

impl NameserverConfig {
    pub fn new(servers: Vec<String>) -> Result<Self, DomainError> {
        if servers.len() < 2 {
            return Err(DomainError::ValidationError(
                "A domain must have at least 2 nameservers".to_string(),
            ));
        }

        let cleaned: Vec<String> = servers.into_iter().map(|s| s.trim().to_string()).collect();
        for ns in &cleaned {
            if ns.is_empty() || !ns.contains('.') {
                return Err(DomainError::ValidationError(format!(
                    "Invalid nameserver hostname '{}'",
                    ns
                )));
            }
        }

        Ok(Self { servers: cleaned })
    }

    pub fn servers(&self) -> &[String] {
        &self.servers
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainRegistration {
    domain: String,
    provider: RegistrarProvider,
    status: DomainStatus,
    contact: ContactInfo,
    nameservers: NameserverConfig,
    auto_renew: bool,
}

impl DomainRegistration {
    pub fn new(
        domain: impl Into<String>,
        provider: RegistrarProvider,
        status: DomainStatus,
        contact: ContactInfo,
        nameservers: NameserverConfig,
        auto_renew: bool,
    ) -> Result<Self, DomainError> {
        let dom = domain.into();
        let trimmed = dom.trim();
        if trimmed.is_empty() || !trimmed.contains('.') || trimmed.starts_with('-') || trimmed.contains("--") {
            return Err(DomainError::ValidationError(format!(
                "Invalid domain name format '{}'",
                trimmed
            )));
        }

        Ok(Self {
            domain: trimmed.to_string(),
            provider,
            status,
            contact,
            nameservers,
            auto_renew,
        })
    }

    pub fn domain(&self) -> &str {
        &self.domain
    }

    pub fn provider(&self) -> RegistrarProvider {
        self.provider
    }

    pub fn status(&self) -> DomainStatus {
        self.status
    }

    pub fn contact(&self) -> &ContactInfo {
        &self.contact
    }

    pub fn nameservers(&self) -> &NameserverConfig {
        &self.nameservers
    }

    pub fn is_auto_renew(&self) -> bool {
        self.auto_renew
    }

    pub fn set_nameservers(&mut self, nameservers: NameserverConfig) {
        self.nameservers = nameservers;
    }

    pub fn set_auto_renew(&mut self, enable: bool) {
        self.auto_renew = enable;
    }
}
