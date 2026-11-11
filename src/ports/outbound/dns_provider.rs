use async_trait::async_trait;

use crate::domain::{
    dns::{DnsRecord, DnsRecordType, DnsZone},
    errors::DomainError,
};

#[async_trait]
pub trait DnsProviderPort: Send + Sync {
    async fn create_zone(&self, zone: &DnsZone) -> Result<(), DomainError>;
    async fn get_zone(&self, domain: &str) -> Result<Option<DnsZone>, DomainError>;
    async fn list_zones(&self) -> Result<Vec<DnsZone>, DomainError>;
    async fn upsert_record(&self, zone_id: &str, record: &DnsRecord) -> Result<(), DomainError>;
    async fn delete_record(&self, zone_id: &str, record_id: &str) -> Result<(), DomainError>;
    async fn verify_propagation(
        &self,
        domain: &str,
        record_type: DnsRecordType,
        expected_value: &str,
    ) -> Result<bool, DomainError>;
}
