use servai::domain::{
    dns::{DnsRecord, DnsRecordType, DnsZone, DnssecStatus, Ttl},
    registrar::{
        ContactInfo, DomainRegistration, DomainStatus, NameserverConfig, RegistrarProvider,
    },
    tls::{CertStatus, ChallengeType, TlsCertificate},
};

#[test]
fn test_dns_record_creation_and_validation() {
    let a_record = DnsRecord::new(
        "rec-01",
        "api.example.com",
        DnsRecordType::A,
        "198.51.100.42",
        Ttl::custom(300).unwrap(),
        None,
    )
    .unwrap();

    assert_eq!(a_record.name(), "api.example.com");
    assert_eq!(a_record.record_type(), DnsRecordType::A);
    assert_eq!(a_record.value(), "198.51.100.42");
    assert_eq!(a_record.ttl().seconds(), 300);

    // MX record with priority
    let mx_record = DnsRecord::new(
        "rec-02",
        "example.com",
        DnsRecordType::MX,
        "mail.example.com",
        Ttl::standard(),
        Some(10),
    )
    .unwrap();

    assert_eq!(mx_record.record_type(), DnsRecordType::MX);
    assert_eq!(mx_record.priority(), Some(10));

    // Validation edge case: invalid IPv4 for A record
    let invalid_a = DnsRecord::new(
        "rec-03",
        "bad.example.com",
        DnsRecordType::A,
        "999.999.999.999",
        Ttl::standard(),
        None,
    );
    assert!(invalid_a.is_err());
}

#[test]
fn test_dns_zone_management_and_records() {
    let mut zone = DnsZone::new("zone-1", "example.com", DnssecStatus::Enabled).unwrap();
    assert_eq!(zone.domain(), "example.com");
    assert_eq!(zone.dnssec_status(), DnssecStatus::Enabled);
    assert_eq!(zone.records().len(), 0);

    let record1 = DnsRecord::new(
        "rec-10",
        "example.com",
        DnsRecordType::A,
        "192.0.2.1",
        Ttl::standard(),
        None,
    )
    .unwrap();
    zone.add_record(record1.clone());
    assert_eq!(zone.records().len(), 1);

    // Duplicate record replacement
    let record1_updated = DnsRecord::new(
        "rec-10",
        "example.com",
        DnsRecordType::A,
        "192.0.2.2",
        Ttl::standard(),
        None,
    )
    .unwrap();
    zone.add_record(record1_updated);
    assert_eq!(zone.records().len(), 1);
    assert_eq!(zone.records()[0].value(), "192.0.2.2");

    zone.remove_record("rec-10");
    assert_eq!(zone.records().len(), 0);
}

#[test]
fn test_domain_registration_and_registrar_management() {
    let contact = ContactInfo::new(
        "Admin",
        "admin@example.com",
        "+33.123456789",
        "Paris",
        "FR",
    )
    .unwrap();

    let ns = NameserverConfig::new(vec![
        "ns1.servai-dns.com".to_string(),
        "ns2.servai-dns.com".to_string(),
    ])
    .unwrap();

    let reg = DomainRegistration::new(
        "example.org",
        RegistrarProvider::Cloudflare,
        DomainStatus::Active,
        contact,
        ns.clone(),
        true, // auto-renew
    )
    .unwrap();

    assert_eq!(reg.domain(), "example.org");
    assert_eq!(reg.provider(), RegistrarProvider::Cloudflare);
    assert_eq!(reg.status(), DomainStatus::Active);
    assert!(reg.is_auto_renew());
    assert_eq!(reg.nameservers().servers().len(), 2);

    // Validation: domain format error
    let invalid_domain = DomainRegistration::new(
        "-invalid--domain",
        RegistrarProvider::Ovh,
        DomainStatus::Active,
        ContactInfo::new("A", "a@a.com", "+1.234", "NY", "US").unwrap(),
        ns,
        false,
    );
    assert!(invalid_domain.is_err());
}

#[test]
fn test_tls_certificate_lifecycle() {
    let cert = TlsCertificate::new(
        "cert-01",
        "api.example.com",
        vec!["example.com".to_string(), "api.example.com".to_string()],
        ChallengeType::Dns01,
        CertStatus::Valid,
        90, // days remaining
    )
    .unwrap();

    assert_eq!(cert.primary_domain(), "api.example.com");
    assert_eq!(cert.san_domains().len(), 2);
    assert_eq!(cert.challenge_type(), ChallengeType::Dns01);
    assert!(!cert.needs_renewal()); // needs renewal when < 30 days

    let expiring_cert = TlsCertificate::new(
        "cert-02",
        "old.example.com",
        vec![],
        ChallengeType::Http01,
        CertStatus::Valid,
        15, // days remaining
    )
    .unwrap();
    assert!(expiring_cert.needs_renewal());
}

#[tokio::test]
async fn test_dns_service_zone_and_record_lifecycle() {
    use servai::{
        adapters::outbound::dns::MockDnsProvider,
        application::dns_service::DnsService,
        ports::inbound::dns_admin::DnsAdminPort,
    };
    use std::sync::Arc;

    let provider = Arc::new(MockDnsProvider::default());
    let service = DnsService::new(provider);

    // 1. Create zone
    let zone = DnsZone::new("zone-cloud", "mycloud.org", DnssecStatus::Enabled).unwrap();
    let created = service.create_dns_zone(zone).await.unwrap();
    assert_eq!(created.domain(), "mycloud.org");

    // 2. Upsert A record
    let record = DnsRecord::new(
        "rec-cloud-1",
        "mycloud.org",
        DnsRecordType::A,
        "198.51.100.88",
        Ttl::fast(),
        None,
    )
    .unwrap();
    let upserted = service.upsert_dns_record("mycloud.org", record).await.unwrap();
    assert_eq!(upserted.value(), "198.51.100.88");

    // 3. Verify propagation
    let is_propagated = service
        .verify_dns_propagation("mycloud.org", DnsRecordType::A, "198.51.100.88")
        .await
        .unwrap();
    assert!(is_propagated);

    // 4. Delete record
    let del_res = service.delete_dns_record("mycloud.org", "rec-cloud-1").await;
    assert!(del_res.is_ok());
}

#[tokio::test]
async fn test_registrar_service_domain_registration_and_nameservers() {
    use servai::{
        adapters::outbound::registrar::MockRegistrarProvider,
        application::registrar_service::RegistrarService,
        ports::inbound::registrar_admin::RegistrarAdminPort,
    };
    use std::sync::Arc;

    let provider = Arc::new(MockRegistrarProvider::default());
    let service = RegistrarService::new(provider);

    // 1. Check availability
    let available = service.check_domain_availability("brand-new-domain.com").await.unwrap();
    assert!(available);

    let contact = ContactInfo::new("DevOps", "ops@bioxus.io", "+33.60000000", "Paris", "FR").unwrap();
    let ns = NameserverConfig::new(vec!["ns1.cloudflare.com".to_string(), "ns2.cloudflare.com".to_string()]).unwrap();
    let reg = DomainRegistration::new(
        "brand-new-domain.com",
        RegistrarProvider::Cloudflare,
        DomainStatus::Active,
        contact,
        ns,
        true,
    )
    .unwrap();

    // 2. Register domain
    let registered = service.register_domain(reg).await.unwrap();
    assert_eq!(registered.domain(), "brand-new-domain.com");

    // 3. Now availability is false
    let now_taken = service.check_domain_availability("brand-new-domain.com").await.unwrap();
    assert!(!now_taken);

    // 4. Update nameservers
    let new_ns = NameserverConfig::new(vec!["ns1.servai.io".to_string(), "ns2.servai.io".to_string()]).unwrap();
    service.update_domain_nameservers("brand-new-domain.com", new_ns).await.unwrap();

    let fetched = service.get_domain("brand-new-domain.com").await.unwrap();
    assert_eq!(fetched.nameservers().servers()[0], "ns1.servai.io");
}

#[tokio::test]
async fn test_server_service_bind_domain_and_auto_ssl() {
    use servai::{
        adapters::outbound::{
            memory::server_store::InMemoryServerStore,
            mock::MockServerExecutor,
        },
        application::server_service::{RegisterServerCommand, ServerService},
        domain::server::SshCredentials,
        ports::inbound::server_admin::ServerAdminPort,
    };
    use std::sync::Arc;

    let server_store = Arc::new(InMemoryServerStore::default());
    let executor = Arc::new(MockServerExecutor::default());
    let service = ServerService::new(server_store, executor);

    let server = service
        .register_server(RegisterServerCommand {
            name: "web-prod-01".to_string(),
            ip: "203.0.113.55".to_string(),
            port: 22,
            credentials: SshCredentials::Agent,
            tags: vec![],
        })
        .await
        .unwrap();
    service.health_check(&server.id()).await.unwrap();

    // Bind domain: automatically registers DNS A record and issues TLS Cert!
    let cert = service.bind_server_domain(&server.id(), "app.servai.net").await.unwrap();
    assert_eq!(cert.primary_domain(), "app.servai.net");
    assert_eq!(cert.status(), CertStatus::Valid);
    assert!(!cert.needs_renewal());
}

