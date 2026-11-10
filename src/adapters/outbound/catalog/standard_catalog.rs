use async_trait::async_trait;

use crate::domain::{
    catalog::{AdminCategory, CatalogItem, ConfigVariable},
    errors::DomainError,
    repository::{GitRef, RepoKind},
};
use crate::ports::outbound::repo_catalog::RepoCatalogPort;

pub struct StandardServerAdminCatalog {
    items: Vec<CatalogItem>,
}

impl Default for StandardServerAdminCatalog {
    fn default() -> Self {
        let items = vec![
            CatalogItem::new(
                "linux-hardening",
                "Linux OS & SSH Security Hardening",
                "Dev-Sec CIS-benchmark compliant system hardening for Linux servers (SSH, kernel sysctl, auditd, limits).",
                AdminCategory::Security,
                "https://github.com/dev-sec/ansible-collection-hardening.git",
                GitRef::Branch("master".to_string()),
                RepoKind::Playbook,
                vec!["Ubuntu".to_string(), "Debian".to_string(), "RHEL".to_string()],
                vec![
                    ConfigVariable::new("ssh_port", "Custom SSH port", Some("22".to_string()), false),
                    ConfigVariable::new("allow_root_ssh", "Permit root login over SSH", Some("no".to_string()), false),
                ],
            ).unwrap(),
            CatalogItem::new(
                "docker-engine-compose",
                "Docker Engine & Docker Compose",
                "Production-ready Docker engine installation with daemon configuration, log rotation, and Compose plugin.",
                AdminCategory::Containers,
                "https://github.com/geerlingguy/ansible-role-docker.git",
                GitRef::Branch("master".to_string()),
                RepoKind::Playbook,
                vec!["Ubuntu".to_string(), "Debian".to_string(), "RHEL".to_string(), "Alpine".to_string()],
                vec![
                    ConfigVariable::new("docker_storage_driver", "Storage driver (overlay2)", Some("overlay2".to_string()), false),
                ],
            ).unwrap(),
            CatalogItem::new(
                "prometheus-node-exporter",
                "Prometheus Node Exporter & Monitoring",
                "Prometheus host metrics collector exposing hardware and OS metrics (CPU, memory, disk, network).",
                AdminCategory::Monitoring,
                "https://github.com/prometheus-community/ansible-node-exporter.git",
                GitRef::Branch("main".to_string()),
                RepoKind::Playbook,
                vec!["Ubuntu".to_string(), "Debian".to_string(), "RHEL".to_string()],
                vec![
                    ConfigVariable::new("exporter_port", "HTTP metrics port", Some("9100".to_string()), false),
                ],
            ).unwrap(),
            CatalogItem::new(
                "fail2ban-ufw-security",
                "Fail2ban & UFW Firewall Protection",
                "Automatic IP banning on SSH brute-force attempts and persistent UFW firewall configuration.",
                AdminCategory::Security,
                "https://github.com/geerlingguy/ansible-role-security.git",
                GitRef::Branch("master".to_string()),
                RepoKind::Playbook,
                vec!["Ubuntu".to_string(), "Debian".to_string()],
                vec![
                    ConfigVariable::new("ban_time", "Ban duration in seconds", Some("3600".to_string()), false),
                    ConfigVariable::new("max_retry", "Maximum login retries before ban", Some("5".to_string()), false),
                ],
            ).unwrap(),
            CatalogItem::new(
                "traefik-proxy",
                "Traefik v3 Edge Router & Reverse Proxy",
                "High-performance cloud-native edge router with automated Let's Encrypt SSL/TLS certificates.",
                AdminCategory::Networking,
                "https://github.com/traefik/traefik.git",
                GitRef::Branch("master".to_string()),
                RepoKind::DockerCompose,
                vec![],
                vec![
                    ConfigVariable::new("acme_email", "Email for Let's Encrypt certificates", None, false),
                    ConfigVariable::new("domain", "Primary server domain name", None, false),
                ],
            ).unwrap(),
            CatalogItem::new(
                "wireguard-vpn",
                "WireGuard High-Speed Secure VPN",
                "Extremely fast and modern VPN server using state-of-the-art cryptography for secure remote management.",
                AdminCategory::Networking,
                "https://github.com/angristan/wireguard-install.git",
                GitRef::Branch("master".to_string()),
                RepoKind::ShellManifest,
                vec!["Ubuntu".to_string(), "Debian".to_string()],
                vec![
                    ConfigVariable::new("vpn_port", "WireGuard UDP listen port", Some("51820".to_string()), false),
                ],
            ).unwrap(),
            CatalogItem::new(
                "borgbackup-server",
                "BorgBackup Encrypted Deduplicating Backup",
                "Deduplicating backup tool with authenticated encryption and compression for server data retention.",
                AdminCategory::Backup,
                "https://github.com/borgbackup/borg.git",
                GitRef::Branch("master".to_string()),
                RepoKind::Playbook,
                vec!["Ubuntu".to_string(), "Debian".to_string()],
                vec![
                    ConfigVariable::new("passphrase", "Encryption passphrase", None, true),
                ],
            ).unwrap(),
            CatalogItem::new(
                "caddy-webserver",
                "Caddy Web Server with Automatic HTTPS",
                "Fast, memory-safe HTTP/2 & HTTP/3 reverse proxy and static web server with fully automated SSL.",
                AdminCategory::Web,
                "https://github.com/caddyserver/caddy.git",
                GitRef::Branch("master".to_string()),
                RepoKind::RawConfig,
                vec!["Ubuntu".to_string(), "Debian".to_string()],
                vec![
                    ConfigVariable::new("domain", "Domain name", None, false),
                ],
            ).unwrap(),
        ];

        Self { items }
    }
}

impl StandardServerAdminCatalog {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl RepoCatalogPort for StandardServerAdminCatalog {
    async fn list_catalog(&self) -> Result<Vec<CatalogItem>, DomainError> {
        Ok(self.items.clone())
    }

    async fn find_catalog_item(&self, id: &str) -> Result<Option<CatalogItem>, DomainError> {
        Ok(self.items.iter().find(|i| i.id() == id).cloned())
    }
}
