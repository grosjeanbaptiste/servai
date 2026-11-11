use async_trait::async_trait;

use crate::domain::{
    catalog::AdminCategory,
    errors::DomainError,
    registry::{AdminPackage, RegistryQuery, RegistrySource, ServerProfilePreset},
};
use crate::ports::outbound::admin_registry::AdminRegistryPort;

pub struct MultiSourceAdminRegistry {
    packages: Vec<AdminPackage>,
    presets: Vec<ServerProfilePreset>,
}

impl Default for MultiSourceAdminRegistry {
    fn default() -> Self {
        let packages = vec![
            AdminPackage::new(
                "dev-sec-hardening",
                "Dev-Sec OS & SSH Hardening",
                "CIS-benchmark baseline hardening for Linux (kernel parameters, limits, sshd security).",
                RegistrySource::AnsibleGalaxy,
                "https://github.com/dev-sec/ansible-collection-hardening.git",
                "master",
                AdminCategory::Security,
                vec![],
                vec!["Ubuntu".to_string(), "Debian".to_string(), "RHEL".to_string()],
                2100,
            ).unwrap(),
            AdminPackage::new(
                "geerlingguy-security",
                "Fail2ban & UFW Firewall",
                "Automated firewall policies and dynamic brute-force attacker IP banning.",
                RegistrySource::AnsibleGalaxy,
                "https://github.com/geerlingguy/ansible-role-security.git",
                "master",
                AdminCategory::Security,
                vec![],
                vec!["Ubuntu".to_string(), "Debian".to_string()],
                1800,
            ).unwrap(),
            AdminPackage::new(
                "geerlingguy-docker",
                "Docker Engine & Compose",
                "Enterprise Docker engine setup with daemon configurations, storage driver and Compose plugin.",
                RegistrySource::AnsibleGalaxy,
                "https://github.com/geerlingguy/ansible-role-docker.git",
                "master",
                AdminCategory::Containers,
                vec![],
                vec!["Ubuntu".to_string(), "Debian".to_string(), "RHEL".to_string()],
                5400,
            ).unwrap(),
            AdminPackage::new(
                "geerlingguy-nginx",
                "Nginx Web & Reverse Proxy",
                "High-performance Nginx server configured for optimal HTTP performance, SSL termination and reverse proxying.",
                RegistrySource::AnsibleGalaxy,
                "https://github.com/geerlingguy/ansible-role-nginx.git",
                "master",
                AdminCategory::Web,
                vec![],
                vec!["Ubuntu".to_string(), "Debian".to_string()],
                3200,
            ).unwrap(),
            AdminPackage::new(
                "geerlingguy-postgresql",
                "PostgreSQL Database Server",
                "Production relational database setup with tuning for server memory, users and automated backups.",
                RegistrySource::AnsibleGalaxy,
                "https://github.com/geerlingguy/ansible-role-postgresql.git",
                "master",
                AdminCategory::Database,
                vec![],
                vec!["Ubuntu".to_string(), "Debian".to_string()],
                2900,
            ).unwrap(),
            AdminPackage::new(
                "geerlingguy-certbot",
                "Let's Encrypt Certbot",
                "Automated TLS/SSL certificate issuance and renewal via Let's Encrypt.",
                RegistrySource::AnsibleGalaxy,
                "https://github.com/geerlingguy/ansible-role-certbot.git",
                "master",
                AdminCategory::Security,
                vec![],
                vec!["Ubuntu".to_string(), "Debian".to_string()],
                2500,
            ).unwrap(),
            AdminPackage::new(
                "prometheus-node-exporter",
                "Prometheus Node Exporter",
                "Host telemetry agent measuring CPU, RAM, disk I/O, network bandwidth and temperature.",
                RegistrySource::GitHubCommunity,
                "https://github.com/prometheus-community/ansible-node-exporter.git",
                "main",
                AdminCategory::Monitoring,
                vec![],
                vec!["Ubuntu".to_string(), "Debian".to_string(), "RHEL".to_string()],
                1100,
            ).unwrap(),
            AdminPackage::new(
                "netdata-agent",
                "Netdata Real-Time Performance Agent",
                "High-fidelity second-by-second metrics monitor with automated anomaly detection.",
                RegistrySource::GitHubCommunity,
                "https://github.com/netdata/netdata.git",
                "master",
                AdminCategory::Monitoring,
                vec![],
                vec!["Ubuntu".to_string(), "Debian".to_string()],
                71000,
            ).unwrap(),
            AdminPackage::new(
                "linuxserver-wireguard",
                "WireGuard VPN Server",
                "Fast, modern, cryptographic VPN server packaged by LinuxServer.io.",
                RegistrySource::LinuxServerIo,
                "https://github.com/linuxserver/docker-wireguard.git",
                "master",
                AdminCategory::Networking,
                vec!["geerlingguy-docker".to_string()],
                vec![],
                4500,
            ).unwrap(),
            AdminPackage::new(
                "linuxserver-nextcloud",
                "Nextcloud Private Cloud Suite",
                "Private cloud file sync, calendar, contacts and office collaboration platform.",
                RegistrySource::LinuxServerIo,
                "https://github.com/linuxserver/docker-nextcloud.git",
                "master",
                AdminCategory::Storage,
                vec!["geerlingguy-docker".to_string()],
                vec![],
                3800,
            ).unwrap(),
            AdminPackage::new(
                "linuxserver-swag",
                "SWAG - Secure Web Application Gateway",
                "Nginx reverse proxy with built-in Certbot, PHP support and fail2ban intrusion prevention.",
                RegistrySource::LinuxServerIo,
                "https://github.com/linuxserver/docker-swag.git",
                "master",
                AdminCategory::Networking,
                vec!["geerlingguy-docker".to_string()],
                vec![],
                2900,
            ).unwrap(),
            AdminPackage::new(
                "traefik-proxy",
                "Traefik v3 Edge Proxy",
                "Cloud-native reverse proxy and load balancer with automatic Let's Encrypt certificates.",
                RegistrySource::GitHubCommunity,
                "https://github.com/traefik/traefik.git",
                "master",
                AdminCategory::Networking,
                vec!["geerlingguy-docker".to_string()],
                vec![],
                53000,
            ).unwrap(),
            AdminPackage::new(
                "caddy-proxy",
                "Caddy Web Server with Automatic TLS",
                "Enterprise HTTPS web server and reverse proxy with zero-configuration SSL/TLS.",
                RegistrySource::GitHubCommunity,
                "https://github.com/caddyserver/caddy.git",
                "master",
                AdminCategory::Web,
                vec![],
                vec!["Ubuntu".to_string(), "Debian".to_string()],
                58000,
            ).unwrap(),
            AdminPackage::new(
                "borgbackup",
                "BorgBackup Deduplicating Backup",
                "Space-efficient, encrypted, authenticated and compressed offsite backups for servers.",
                RegistrySource::CuratedServai,
                "https://github.com/borgbackup/borg.git",
                "master",
                AdminCategory::Backup,
                vec![],
                vec!["Ubuntu".to_string(), "Debian".to_string()],
                11000,
            ).unwrap(),
            AdminPackage::new(
                "crowdsec",
                "CrowdSec Collaborative Intrusion Prevention",
                "Modern security engine parsing logs to detect and block malicious traffic via community threat intel.",
                RegistrySource::GitHubCommunity,
                "https://github.com/crowdsecurity/crowdsec.git",
                "master",
                AdminCategory::Security,
                vec![],
                vec!["Ubuntu".to_string(), "Debian".to_string()],
                10500,
            ).unwrap(),
            AdminPackage::new(
                "restic-backup",
                "Restic Fast Secure Backup",
                "Secure backups supporting cloud object storage (S3, B2, Azure, SFTP) out of the box.",
                RegistrySource::GitHubCommunity,
                "https://github.com/restic/restic.git",
                "master",
                AdminCategory::Backup,
                vec![],
                vec!["Ubuntu".to_string(), "Debian".to_string()],
                28000,
            ).unwrap(),
            AdminPackage::new(
                "k3s-cluster",
                "K3s Lightweight Kubernetes",
                "Highly available certified lightweight Kubernetes cluster orchestrator for edge and cloud servers.",
                RegistrySource::GitHubCommunity,
                "https://github.com/k3s-io/k3s-ansible.git",
                "master",
                AdminCategory::Containers,
                vec![],
                vec!["Ubuntu".to_string(), "Debian".to_string()],
                8500,
            ).unwrap(),
            AdminPackage::new(
                "vaultwarden-server",
                "Vaultwarden Password Manager",
                "Lightweight Bitwarden-compatible password and secret management server.",
                RegistrySource::GitHubCommunity,
                "https://github.com/dani-garcia/vaultwarden.git",
                "main",
                AdminCategory::Security,
                vec!["geerlingguy-docker".to_string()],
                vec![],
                39000,
            ).unwrap(),
            AdminPackage::new(
                "uptime-kuma",
                "Uptime Kuma Service Monitor",
                "Self-hosted status page and uptime monitoring tool with multi-channel alerts.",
                RegistrySource::GitHubCommunity,
                "https://github.com/louislam/uptime-kuma.git",
                "master",
                AdminCategory::Monitoring,
                vec!["geerlingguy-docker".to_string()],
                vec![],
                66000,
            ).unwrap(),
            AdminPackage::new(
                "grafana-server",
                "Grafana Observability Dashboards",
                "Open-source data visualization and metrics dashboarding platform.",
                RegistrySource::GitHubCommunity,
                "https://github.com/grafana/grafana.git",
                "main",
                AdminCategory::Monitoring,
                vec!["geerlingguy-docker".to_string()],
                vec![],
                63000,
            ).unwrap(),
            AdminPackage::new(
                "dotfiles-stow",
                "Dotfiles & Machine State Provisioning",
                "Personalized dotfiles, shell environment, and GNU Stow manifests.",
                RegistrySource::GitHubCommunity,
                "https://github.com/grosjeanbaptiste/dotfiles.git",
                "main",
                AdminCategory::Security,
                vec![],
                vec!["Ubuntu".to_string(), "Debian".to_string(), "Darwin".to_string()],
                50,
            ).unwrap(),
            AdminPackage::new(
                "ss-sensor",
                "SS High-Precision System Sensor",
                "Modular diagnostic and administration sensor for kernel, network, memory, and processes.",
                RegistrySource::GitHubCommunity,
                "https://github.com/grosjeanbaptiste/ss.git",
                "main",
                AdminCategory::Monitoring,
                vec![],
                vec!["Ubuntu".to_string(), "Debian".to_string(), "Darwin".to_string()],
                100,
            ).unwrap(),
        ];

        let presets = vec![
            ServerProfilePreset::new(
                "secure-web-baseline",
                "Secure Web Baseline",
                "Production Linux hardening + UFW/Fail2ban firewall + Caddy auto-HTTPS reverse proxy.",
                vec![
                    "dev-sec-hardening".to_string(),
                    "geerlingguy-security".to_string(),
                    "caddy-proxy".to_string(),
                ],
            ).unwrap(),
            ServerProfilePreset::new(
                "docker-application-host",
                "Docker Application Host",
                "Security hardening + Docker engine & Compose + Traefik v3 edge reverse proxy.",
                vec![
                    "dev-sec-hardening".to_string(),
                    "geerlingguy-security".to_string(),
                    "geerlingguy-docker".to_string(),
                    "traefik-proxy".to_string(),
                ],
            ).unwrap(),
            ServerProfilePreset::new(
                "observability-node",
                "Full Observability & Monitoring Node",
                "Prometheus Node Exporter + Netdata real-time agent + Grafana visual dashboards.",
                vec![
                    "prometheus-node-exporter".to_string(),
                    "netdata-agent".to_string(),
                    "geerlingguy-docker".to_string(),
                    "grafana-server".to_string(),
                ],
            ).unwrap(),
            ServerProfilePreset::new(
                "homelab-private-cloud",
                "Homelab & Private Cloud Suite",
                "Hardened server + Docker + WireGuard VPN + Nextcloud files + Vaultwarden vault.",
                vec![
                    "dev-sec-hardening".to_string(),
                    "geerlingguy-docker".to_string(),
                    "linuxserver-wireguard".to_string(),
                    "linuxserver-nextcloud".to_string(),
                    "vaultwarden-server".to_string(),
                ],
            ).unwrap(),
            ServerProfilePreset::new(
                "gibs-developer-node",
                "GIBS Developer Node",
                "Hardened server + Docker + Dotfiles (Stow) + System Status (ss) sensor + WireGuard VPN.",
                vec![
                    "dev-sec-hardening".to_string(),
                    "geerlingguy-docker".to_string(),
                    "dotfiles-stow".to_string(),
                    "ss-sensor".to_string(),
                    "linuxserver-wireguard".to_string(),
                ],
            ).unwrap(),
        ];

        Self { packages, presets }
    }
}

impl MultiSourceAdminRegistry {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl AdminRegistryPort for MultiSourceAdminRegistry {
    async fn search(&self, query: &RegistryQuery) -> Result<Vec<AdminPackage>, DomainError> {
        let results = self
            .packages
            .iter()
            .filter(|pkg| query.matches(pkg))
            .cloned()
            .collect();
        Ok(results)
    }

    async fn get_package(&self, id: &str) -> Result<Option<AdminPackage>, DomainError> {
        Ok(self.packages.iter().find(|p| p.id() == id).cloned())
    }

    async fn list_presets(&self) -> Result<Vec<ServerProfilePreset>, DomainError> {
        Ok(self.presets.clone())
    }

    async fn resolve_dependencies(&self, package_id: &str) -> Result<Vec<AdminPackage>, DomainError> {
        let root = self
            .get_package(package_id)
            .await?
            .ok_or_else(|| DomainError::NotFound {
                entity: "AdminPackage",
                id: package_id.to_string(),
            })?;

        let mut resolved = Vec::new();
        for dep_id in root.dependencies() {
            let dep_pkg = self
                .get_package(dep_id)
                .await?
                .ok_or_else(|| DomainError::NotFound {
                    entity: "AdminPackage dependency",
                    id: dep_id.clone(),
                })?;
            if !resolved.iter().any(|p: &AdminPackage| p.id() == dep_pkg.id()) {
                resolved.push(dep_pkg);
            }
        }
        resolved.push(root);

        Ok(resolved)
    }
}
