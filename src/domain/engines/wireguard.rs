use serde::{Deserialize, Serialize};

use crate::domain::errors::DomainError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireGuardPeer {
    pub public_key: String,
    pub preshared_key: Option<String>,
    pub endpoint: Option<String>,
    pub allowed_ips: Vec<String>,
    pub persistent_keepalive: Option<u16>,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub latest_handshake_epoch: Option<u64>,
}

impl WireGuardPeer {
    pub fn new(
        public_key: impl Into<String>,
        preshared_key: Option<String>,
        endpoint: Option<String>,
        allowed_ips: Vec<String>,
        persistent_keepalive: Option<u16>,
    ) -> Result<Self, DomainError> {
        let pubkey = public_key.into();
        let trimmed_pubkey = pubkey.trim();
        if trimmed_pubkey.is_empty() {
            return Err(DomainError::ValidationError(
                "WireGuard peer public key cannot be empty".to_string(),
            ));
        }

        if allowed_ips.is_empty() {
            return Err(DomainError::ValidationError(
                "WireGuard peer must specify at least one allowed IP".to_string(),
            ));
        }

        Ok(Self {
            public_key: trimmed_pubkey.to_string(),
            preshared_key,
            endpoint,
            allowed_ips,
            persistent_keepalive,
            rx_bytes: 0,
            tx_bytes: 0,
            latest_handshake_epoch: None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireGuardInterface {
    name: String,
    private_key: String,
    public_key: String,
    listen_port: u16,
    address: String,
    peers: Vec<WireGuardPeer>,
}

impl WireGuardInterface {
    pub fn new(
        name: impl Into<String>,
        private_key: impl Into<String>,
        public_key: impl Into<String>,
        listen_port: u16,
        address: impl Into<String>,
        peers: Vec<WireGuardPeer>,
    ) -> Result<Self, DomainError> {
        let name_str = name.into();
        let trimmed_name = name_str.trim();
        if trimmed_name.is_empty() {
            return Err(DomainError::ValidationError(
                "WireGuard interface name cannot be empty".to_string(),
            ));
        }

        let privkey = private_key.into();
        if privkey.trim().is_empty() {
            return Err(DomainError::ValidationError(
                "WireGuard private key cannot be empty".to_string(),
            ));
        }

        let addr = address.into();
        if addr.trim().is_empty() {
            return Err(DomainError::ValidationError(
                "WireGuard address cannot be empty".to_string(),
            ));
        }

        Ok(Self {
            name: trimmed_name.to_string(),
            private_key: privkey.trim().to_string(),
            public_key: public_key.into().trim().to_string(),
            listen_port,
            address: addr.trim().to_string(),
            peers,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn private_key(&self) -> &str {
        &self.private_key
    }

    pub fn public_key(&self) -> &str {
        &self.public_key
    }

    pub fn listen_port(&self) -> u16 {
        self.listen_port
    }

    pub fn address(&self) -> &str {
        &self.address
    }

    pub fn peers(&self) -> &[WireGuardPeer] {
        &self.peers
    }

    pub fn add_peer(&mut self, peer: WireGuardPeer) {
        self.peers.retain(|p| p.public_key != peer.public_key);
        self.peers.push(peer);
    }

    pub fn remove_peer(&mut self, pubkey: &str) {
        self.peers.retain(|p| p.public_key != pubkey);
    }

    pub fn render_wg_quick_config(&self) -> String {
        let mut out = String::new();
        out.push_str("[Interface]\n");
        out.push_str(&format!("PrivateKey = {}\n", self.private_key));
        out.push_str(&format!("Address = {}\n", self.address));
        if self.listen_port > 0 {
            out.push_str(&format!("ListenPort = {}\n", self.listen_port));
        }

        for peer in &self.peers {
            out.push_str("\n[Peer]\n");
            out.push_str(&format!("PublicKey = {}\n", peer.public_key));
            if let Some(ref psk) = peer.preshared_key {
                out.push_str(&format!("PresharedKey = {}\n", psk));
            }
            if let Some(ref endpoint) = peer.endpoint {
                out.push_str(&format!("Endpoint = {}\n", endpoint));
            }
            out.push_str(&format!("AllowedIPs = {}\n", peer.allowed_ips.join(", ")));
            if let Some(keepalive) = peer.persistent_keepalive {
                out.push_str(&format!("PersistentKeepalive = {}\n", keepalive));
            }
        }

        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireGuardDeviceStatus {
    pub interface: String,
    pub public_key: String,
    pub listen_port: u16,
    pub active_peers: usize,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub is_up: bool,
}

impl WireGuardDeviceStatus {
    pub fn new(
        interface: impl Into<String>,
        public_key: impl Into<String>,
        listen_port: u16,
        active_peers: usize,
        rx_bytes: u64,
        tx_bytes: u64,
        is_up: bool,
    ) -> Self {
        Self {
            interface: interface.into(),
            public_key: public_key.into(),
            listen_port,
            active_peers,
            rx_bytes,
            tx_bytes,
            is_up,
        }
    }
}
