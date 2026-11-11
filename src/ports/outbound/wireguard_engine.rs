use async_trait::async_trait;

use crate::domain::{
    engines::wireguard::{WireGuardDeviceStatus, WireGuardInterface, WireGuardPeer},
    errors::DomainError,
    server::Server,
};

#[async_trait]
pub trait WireGuardEnginePort: Send + Sync {
    async fn configure_interface(&self, server: &Server, iface: &WireGuardInterface) -> Result<(), DomainError>;
    async fn add_peer(&self, server: &Server, iface_name: &str, peer: &WireGuardPeer) -> Result<(), DomainError>;
    async fn remove_peer(&self, server: &Server, iface_name: &str, peer_pubkey: &str) -> Result<(), DomainError>;
    async fn get_device_status(&self, server: &Server, iface_name: &str) -> Result<WireGuardDeviceStatus, DomainError>;
    async fn list_peers(&self, server: &Server, iface_name: &str) -> Result<Vec<WireGuardPeer>, DomainError>;
    async fn generate_keypair(&self) -> Result<(String, String), DomainError>;
}
