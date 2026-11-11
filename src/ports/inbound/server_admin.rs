use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::domain::{
    engines::{
        ansible::{AnsibleFacts, AnsibleModuleResult},
        wireguard::{WireGuardDeviceStatus, WireGuardInterface, WireGuardPeer},
    },
    errors::DomainError,
    server::{Server, ServerId, ServerMetrics, ServerStatus, ServiceAction, ServiceStatus, SshCredentials},
};
use crate::ports::outbound::server_executor::CommandResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterServerCommand {
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub credentials: SshCredentials,
    pub tags: Vec<String>,
}

#[async_trait]
pub trait ServerAdminPort: Send + Sync {
    async fn register_server(&self, cmd: RegisterServerCommand) -> Result<Server, DomainError>;
    async fn get_server(&self, id: &ServerId) -> Result<Server, DomainError>;
    async fn list_servers(&self) -> Result<Vec<Server>, DomainError>;
    async fn health_check(&self, id: &ServerId) -> Result<ServerStatus, DomainError>;
    async fn set_maintenance(&self, id: &ServerId, enable: bool) -> Result<Server, DomainError>;
    async fn execute_on_server(&self, id: &ServerId, command: &str) -> Result<CommandResult, DomainError>;
    async fn manage_service(
        &self,
        id: &ServerId,
        service: &str,
        action: ServiceAction,
    ) -> Result<ServiceStatus, DomainError>;
    async fn get_metrics(&self, id: &ServerId) -> Result<ServerMetrics, DomainError>;
    async fn reboot_server(&self, id: &ServerId) -> Result<(), DomainError>;
    async fn run_ansible_module(
        &self,
        id: &ServerId,
        module: &str,
        args: &str,
    ) -> Result<AnsibleModuleResult, DomainError>;
    async fn gather_ansible_facts(&self, id: &ServerId) -> Result<AnsibleFacts, DomainError>;

    // WireGuard VPN administration
    async fn configure_wireguard(
        &self,
        id: &ServerId,
        iface: WireGuardInterface,
    ) -> Result<(), DomainError>;
    async fn add_wireguard_peer(
        &self,
        id: &ServerId,
        iface_name: &str,
        peer: WireGuardPeer,
    ) -> Result<(), DomainError>;
    async fn remove_wireguard_peer(
        &self,
        id: &ServerId,
        iface_name: &str,
        peer_pubkey: &str,
    ) -> Result<(), DomainError>;
    async fn get_wireguard_status(
        &self,
        id: &ServerId,
        iface_name: &str,
    ) -> Result<WireGuardDeviceStatus, DomainError>;
    async fn list_wireguard_peers(
        &self,
        id: &ServerId,
        iface_name: &str,
    ) -> Result<Vec<WireGuardPeer>, DomainError>;
    async fn generate_wireguard_keypair(&self) -> Result<(String, String), DomainError>;
}
