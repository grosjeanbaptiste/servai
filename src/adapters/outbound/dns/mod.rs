use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::domain::{
    dns::{DnsRecord, DnsRecordType, DnsZone, DnssecStatus},
    errors::DomainError,
};
use crate::ports::outbound::dns_provider::DnsProviderPort;

#[derive(Clone)]
pub struct MockDnsProvider {
    pub should_fail: bool,
    zones: Arc<RwLock<HashMap<String, DnsZone>>>,
}

impl Default for MockDnsProvider {
    fn default() -> Self {
        let mut map = HashMap::new();
        let mut sample_zone = DnsZone::new("zone-sample-1", "servai.net", DnssecStatus::Enabled).unwrap();
        sample_zone.add_record(
            DnsRecord::new(
                "rec-ns1",
                "servai.net",
                DnsRecordType::NS,
                "ns1.servai.net",
                crate::domain::dns::Ttl::standard(),
                None,
            )
            .unwrap(),
        );
        sample_zone.add_record(
            DnsRecord::new(
                "rec-ns2",
                "servai.net",
                DnsRecordType::NS,
                "ns2.servai.net",
                crate::domain::dns::Ttl::standard(),
                None,
            )
            .unwrap(),
        );
        map.insert("servai.net".to_string(), sample_zone);

        Self {
            should_fail: false,
            zones: Arc::new(RwLock::new(map)),
        }
    }
}

#[async_trait]
impl DnsProviderPort for MockDnsProvider {
    async fn create_zone(&self, zone: &DnsZone) -> Result<(), DomainError> {
        if self.should_fail {
            return Err(DomainError::ExecutionFailed {
                reason: format!("Failed to create DNS zone for {}", zone.domain()),
                details: None,
            });
        }
        self.zones
            .write()
            .unwrap()
            .insert(zone.domain().to_string(), zone.clone());
        Ok(())
    }

    async fn get_zone(&self, domain: &str) -> Result<Option<DnsZone>, DomainError> {
        Ok(self.zones.read().unwrap().get(domain).cloned())
    }

    async fn list_zones(&self) -> Result<Vec<DnsZone>, DomainError> {
        Ok(self.zones.read().unwrap().values().cloned().collect())
    }

    async fn upsert_record(&self, zone_id: &str, record: &DnsRecord) -> Result<(), DomainError> {
        if self.should_fail {
            return Err(DomainError::ExecutionFailed {
                reason: format!("Failed to upsert DNS record {} in zone {}", record.name(), zone_id),
                details: None,
            });
        }
        let mut map = self.zones.write().unwrap();
        // Match by zone id or domain
        let zone = map
            .values_mut()
            .find(|z| z.id() == zone_id || z.domain() == zone_id)
            .ok_or_else(|| DomainError::NotFound {
                entity: "DnsZone",
                id: zone_id.to_string(),
            })?;
        zone.add_record(record.clone());
        Ok(())
    }

    async fn delete_record(&self, zone_id: &str, record_id: &str) -> Result<(), DomainError> {
        if self.should_fail {
            return Err(DomainError::ExecutionFailed {
                reason: format!("Failed to delete DNS record {} in zone {}", record_id, zone_id),
                details: None,
            });
        }
        let mut map = self.zones.write().unwrap();
        let zone = map
            .values_mut()
            .find(|z| z.id() == zone_id || z.domain() == zone_id)
            .ok_or_else(|| DomainError::NotFound {
                entity: "DnsZone",
                id: zone_id.to_string(),
            })?;
        zone.remove_record(record_id);
        Ok(())
    }

    async fn verify_propagation(
        &self,
        domain: &str,
        record_type: DnsRecordType,
        expected_value: &str,
    ) -> Result<bool, DomainError> {
        let map = self.zones.read().unwrap();
        for zone in map.values() {
            if zone.domain() == domain || domain.ends_with(zone.domain()) {
                for rec in zone.records() {
                    let matches_name = rec.name() == domain
                        || zone.domain() == domain
                        || format!("{}.{}", rec.name(), zone.domain()) == domain;
                    if matches_name
                        && rec.record_type() == record_type
                        && rec.value() == expected_value
                    {
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }
}
