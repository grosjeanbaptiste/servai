use async_trait::async_trait;

use crate::domain::{
    dns::{DnsRecord, DnsRecordType, DnsZone},
    errors::DomainError,
};

#[async_trait]
pub trait DnsAdminPort: Send + Sync {
    async fn create_dns_zone(&self, zone: DnsZone) -> Result<DnsZone, DomainError>;
    async fn get_dns_zone(&self, domain: &str) -> Result<DnsZone, DomainError>;
    async fn list_dns_zones(&self) -> Result<Vec<DnsZone>, DomainError>;
    async fn upsert_dns_record(&self, zone_id: &str, record: DnsRecord) -> Result<DnsRecord, DomainError>;
    async fn delete_dns_record(&self, zone_id: &str, record_id: &str) -> Result<(), DomainError>;
    async fn verify_dns_propagation(
        &self,
        domain: &str,
        record_type: DnsRecordType,
        expected_value: &str,
    ) -> Result<bool, DomainError>;
}
