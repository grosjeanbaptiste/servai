use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, Ipv6Addr};

use crate::domain::errors::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DnsRecordType {
    A,
    AAAA,
    CNAME,
    MX,
    TXT,
    SRV,
    CAA,
    NS,
    PTR,
    SOA,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ttl(u32);

impl Ttl {
    pub fn custom(seconds: u32) -> Result<Self, DomainError> {
        if !(60..=86400).contains(&seconds) {
            return Err(DomainError::ValidationError(
                "TTL must be between 60 and 86400 seconds".to_string(),
            ));
        }
        Ok(Self(seconds))
    }

    pub fn standard() -> Self {
        Self(3600)
    }

    pub fn fast() -> Self {
        Self(300)
    }

    pub fn seconds(&self) -> u32 {
        self.0
    }
}

impl Default for Ttl {
    fn default() -> Self {
        Self::standard()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsRecord {
    id: String,
    name: String,
    record_type: DnsRecordType,
    value: String,
    ttl: Ttl,
    priority: Option<u16>,
}

impl DnsRecord {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        record_type: DnsRecordType,
        value: impl Into<String>,
        ttl: Ttl,
        priority: Option<u16>,
    ) -> Result<Self, DomainError> {
        let name_str = name.into();
        let trimmed_name = name_str.trim();
        if trimmed_name.is_empty() {
            return Err(DomainError::ValidationError(
                "DNS record name cannot be empty".to_string(),
            ));
        }

        let val_str = value.into();
        let trimmed_val = val_str.trim();
        if trimmed_val.is_empty() {
            return Err(DomainError::ValidationError(
                "DNS record value cannot be empty".to_string(),
            ));
        }

        // Validate IP formats for A and AAAA records
        match record_type {
            DnsRecordType::A => {
                if trimmed_val.parse::<Ipv4Addr>().is_err() {
                    return Err(DomainError::ValidationError(format!(
                        "Invalid IPv4 address '{}' for A record",
                        trimmed_val
                    )));
                }
            }
            DnsRecordType::AAAA => {
                if trimmed_val.parse::<Ipv6Addr>().is_err() {
                    return Err(DomainError::ValidationError(format!(
                        "Invalid IPv6 address '{}' for AAAA record",
                        trimmed_val
                    )));
                }
            }
            _ => {}
        }

        Ok(Self {
            id: id.into(),
            name: trimmed_name.to_string(),
            record_type,
            value: trimmed_val.to_string(),
            ttl,
            priority,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn record_type(&self) -> DnsRecordType {
        self.record_type
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn ttl(&self) -> Ttl {
        self.ttl
    }

    pub fn priority(&self) -> Option<u16> {
        self.priority
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnssecStatus {
    Enabled,
    Disabled,
    Pending,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsZone {
    id: String,
    domain: String,
    dnssec_status: DnssecStatus,
    records: Vec<DnsRecord>,
}

impl DnsZone {
    pub fn new(
        id: impl Into<String>,
        domain: impl Into<String>,
        dnssec_status: DnssecStatus,
    ) -> Result<Self, DomainError> {
        let dom = domain.into();
        let trimmed_dom = dom.trim();
        if trimmed_dom.is_empty() || !trimmed_dom.contains('.') {
            return Err(DomainError::ValidationError(format!(
                "Invalid domain name '{}' for DNS zone",
                trimmed_dom
            )));
        }

        Ok(Self {
            id: id.into(),
            domain: trimmed_dom.to_string(),
            dnssec_status,
            records: Vec::new(),
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn domain(&self) -> &str {
        &self.domain
    }

    pub fn dnssec_status(&self) -> DnssecStatus {
        self.dnssec_status
    }

    pub fn records(&self) -> &[DnsRecord] {
        &self.records
    }

    pub fn add_record(&mut self, record: DnsRecord) {
        self.records.retain(|r| r.id() != record.id());
        self.records.push(record);
    }

    pub fn remove_record(&mut self, record_id: &str) {
        self.records.retain(|r| r.id() != record_id);
    }

    pub fn find_records_by_name(&self, name: &str) -> Vec<&DnsRecord> {
        self.records.iter().filter(|r| r.name() == name).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnsPropagationStatus {
    Propagated,
    Pending,
    Failed,
}
