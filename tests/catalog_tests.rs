use servai::domain::{
    catalog::{AdminCategory, CatalogItem, ConfigVariable},
    errors::DomainError,
    repository::{GitRef, RepoKind},
    server::ServerSpec,
};

#[test]
fn test_catalog_item_creation_and_os_compatibility() {
    let item = CatalogItem::new(
        "security-hardening",
        "Linux OS & SSH Security Hardening (Dev-Sec)",
        "Automated CIS benchmark security hardening for Linux servers (SSH, sysctl, limits, firewall)",
        AdminCategory::Security,
        "https://github.com/dev-sec/ansible-collection-hardening.git",
        GitRef::Branch("master".to_string()),
        RepoKind::Playbook,
        vec!["Ubuntu".to_string(), "Debian".to_string(), "RHEL".to_string()],
        vec![
            ConfigVariable::new("ssh_port", "Custom SSH port", Some("22".to_string()), false),
            ConfigVariable::new("allow_root_ssh", "Permit root login over SSH", Some("no".to_string()), false),
        ],
    ).expect("valid catalog item");

    assert_eq!(item.id(), "security-hardening");
    assert_eq!(item.category(), AdminCategory::Security);
    assert_eq!(item.required_variables().len(), 2);

    let ubuntu_spec = ServerSpec {
        os: "Ubuntu 24.04 LTS".to_string(),
        arch: "x86_64".to_string(),
        cpu_cores: 4,
        ram_mb: 8192,
    };
    assert!(item.is_compatible_with(&ubuntu_spec));

    let windows_spec = ServerSpec {
        os: "Windows Server 2022".to_string(),
        arch: "x86_64".to_string(),
        cpu_cores: 8,
        ram_mb: 16384,
    };
    assert!(!item.is_compatible_with(&windows_spec));
}

#[test]
fn test_catalog_validation_errors() {
    // Empty ID
    assert!(matches!(
        CatalogItem::new(
            "",
            "Title",
            "Desc",
            AdminCategory::Monitoring,
            "https://github.com/prometheus/node_exporter.git",
            GitRef::Branch("main".to_string()),
            RepoKind::DockerCompose,
            vec![],
            vec![],
        ),
        Err(DomainError::ValidationError(_))
    ));

    // Invalid URL
    assert!(matches!(
        CatalogItem::new(
            "bad-url-item",
            "Title",
            "Desc",
            AdminCategory::Monitoring,
            "ftp://invalid-url.com",
            GitRef::Branch("main".to_string()),
            RepoKind::DockerCompose,
            vec![],
            vec![],
        ),
        Err(DomainError::ValidationError(_))
    ));
}
