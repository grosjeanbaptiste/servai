use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::domain::{
    errors::DomainError,
    tls::{CertStatus, ChallengeType, TlsCertificate},
};
use crate::ports::outbound::tls_manager::TlsManagerPort;

#[derive(Clone)]
pub struct MockTlsManager {
    pub should_fail: bool,
    certs: Arc<RwLock<HashMap<String, TlsCertificate>>>,
}

impl Default for MockTlsManager {
    fn default() -> Self {
        let mut map = HashMap::new();
        let cert = TlsCertificate::new(
            "cert-initial",
            "servai.net",
            vec!["*.servai.net".to_string()],
            ChallengeType::Dns01,
            CertStatus::Valid,
            89,
        )
        .unwrap();
        map.insert("servai.net".to_string(), cert);

        Self {
            should_fail: false,
            certs: Arc::new(RwLock::new(map)),
        }
    }
}

#[async_trait]
impl TlsManagerPort for MockTlsManager {
    async fn request_certificate(
        &self,
        primary_domain: &str,
        san_domains: Vec<String>,
        challenge: ChallengeType,
    ) -> Result<TlsCertificate, DomainError> {
        if self.should_fail {
            return Err(DomainError::ExecutionFailed {
                reason: format!("ACME challenge failed for domain {}", primary_domain),
                details: None,
            });
        }

        let cert = TlsCertificate::new(
            format!("cert-{}", primary_domain.replace('.', "-")),
            primary_domain,
            san_domains,
            challenge,
            CertStatus::Valid,
            90,
        )?;

        self.certs
            .write()
            .unwrap()
            .insert(primary_domain.to_string(), cert.clone());
        Ok(cert)
    }

    async fn get_certificate(&self, domain: &str) -> Result<Option<TlsCertificate>, DomainError> {
        Ok(self.certs.read().unwrap().get(domain).cloned())
    }

    async fn renew_certificate(&self, domain: &str) -> Result<TlsCertificate, DomainError> {
        if self.should_fail {
            return Err(DomainError::ExecutionFailed {
                reason: format!("Failed to renew certificate for {}", domain),
                details: None,
            });
        }

        let mut map = self.certs.write().unwrap();
        let cert = map.get_mut(domain).ok_or_else(|| DomainError::NotFound {
            entity: "TlsCertificate",
            id: domain.to_string(),
        })?;

        let renewed = TlsCertificate::new(
            cert.id(),
            cert.primary_domain(),
            cert.san_domains().to_vec(),
            cert.challenge_type(),
            CertStatus::Valid,
            90,
        )?;

        *cert = renewed.clone();
        Ok(renewed)
    }
}
