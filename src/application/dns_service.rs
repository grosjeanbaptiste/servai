use async_trait::async_trait;
use std::sync::Arc;

use crate::domain::{
    dns::{DnsRecord, DnsRecordType, DnsZone},
    errors::DomainError,
};
use crate::ports::{inbound::dns_admin::DnsAdminPort, outbound::dns_provider::DnsProviderPort};

pub struct DnsService {
    dns_provider: Arc<dyn DnsProviderPort>,
}

impl DnsService {
    pub fn new(dns_provider: Arc<dyn DnsProviderPort>) -> Self {
        Self { dns_provider }
    }
}

#[async_trait]
impl DnsAdminPort for DnsService {
    async fn create_dns_zone(&self, zone: DnsZone) -> Result<DnsZone, DomainError> {
        self.dns_provider.create_zone(&zone).await?;
        Ok(zone)
    }

    async fn get_dns_zone(&self, domain: &str) -> Result<DnsZone, DomainError> {
        self.dns_provider
            .get_zone(domain)
            .await?
            .ok_or_else(|| DomainError::NotFound {
                entity: "DnsZone",
                id: domain.to_string(),
            })
    }

    async fn list_dns_zones(&self) -> Result<Vec<DnsZone>, DomainError> {
        self.dns_provider.list_zones().await
    }

    async fn upsert_dns_record(&self, zone_id: &str, record: DnsRecord) -> Result<DnsRecord, DomainError> {
        self.dns_provider.upsert_record(zone_id, &record).await?;
        Ok(record)
    }

    async fn delete_dns_record(&self, zone_id: &str, record_id: &str) -> Result<(), DomainError> {
        self.dns_provider.delete_record(zone_id, record_id).await
    }

    async fn verify_dns_propagation(
        &self,
        domain: &str,
        record_type: DnsRecordType,
        expected_value: &str,
    ) -> Result<bool, DomainError> {
        self.dns_provider
            .verify_propagation(domain, record_type, expected_value)
            .await
    }
}
