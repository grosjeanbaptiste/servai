use async_trait::async_trait;

use crate::domain::{
    errors::DomainError,
    tls::{ChallengeType, TlsCertificate},
};

#[async_trait]
pub trait TlsManagerPort: Send + Sync {
    async fn request_certificate(
        &self,
        primary_domain: &str,
        san_domains: Vec<String>,
        challenge: ChallengeType,
    ) -> Result<TlsCertificate, DomainError>;
    async fn get_certificate(&self, domain: &str) -> Result<Option<TlsCertificate>, DomainError>;
    async fn renew_certificate(&self, domain: &str) -> Result<TlsCertificate, DomainError>;
}
