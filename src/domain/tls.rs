use serde::{Deserialize, Serialize};

use crate::domain::errors::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChallengeType {
    Dns01,
    Http01,
    TlsAlpn01,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CertStatus {
    Valid,
    Pending,
    Revoked,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TlsCertificate {
    id: String,
    primary_domain: String,
    san_domains: Vec<String>,
    challenge_type: ChallengeType,
    status: CertStatus,
    days_remaining: u32,
}

impl TlsCertificate {
    pub fn new(
        id: impl Into<String>,
        primary_domain: impl Into<String>,
        san_domains: Vec<String>,
        challenge_type: ChallengeType,
        status: CertStatus,
        days_remaining: u32,
    ) -> Result<Self, DomainError> {
        let domain_str = primary_domain.into();
        let trimmed_dom = domain_str.trim();
        if trimmed_dom.is_empty() || !trimmed_dom.contains('.') {
            return Err(DomainError::ValidationError(format!(
                "Invalid primary domain '{}' for TLS certificate",
                trimmed_dom
            )));
        }

        Ok(Self {
            id: id.into(),
            primary_domain: trimmed_dom.to_string(),
            san_domains,
            challenge_type,
            status,
            days_remaining,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn primary_domain(&self) -> &str {
        &self.primary_domain
    }

    pub fn san_domains(&self) -> &[String] {
        &self.san_domains
    }

    pub fn challenge_type(&self) -> ChallengeType {
        self.challenge_type
    }

    pub fn status(&self) -> CertStatus {
        self.status
    }

    pub fn days_remaining(&self) -> u32 {
        self.days_remaining
    }

    pub fn needs_renewal(&self) -> bool {
        self.days_remaining <= 30
    }
}
