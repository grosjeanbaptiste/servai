use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::process::Command;

use crate::domain::{
    engines::wireguard::{WireGuardDeviceStatus, WireGuardInterface, WireGuardPeer},
    errors::DomainError,
    server::Server,
};
use crate::ports::outbound::wireguard_engine::WireGuardEnginePort;

#[derive(Clone)]
pub struct MockWireGuardEngine {
    pub should_fail: bool,
    interfaces: Arc<RwLock<HashMap<String, WireGuardInterface>>>,
}

impl Default for MockWireGuardEngine {
    fn default() -> Self {
        let mut map = HashMap::new();
        let peer = WireGuardPeer::new(
            "mock-client-pubkey-abcdef123456=",
            None,
            Some("198.51.100.20:51820".to_string()),
            vec!["10.0.0.2/32".to_string()],
            Some(25),
        )
        .unwrap();

        let iface = WireGuardInterface::new(
            "wg0",
            "mock-server-privkey-1234567890=",
            "mock-server-pubkey-1234567890=",
            51820,
            "10.0.0.1/24",
            vec![peer],
        )
        .unwrap();

        map.insert("wg0".to_string(), iface);

        Self {
            should_fail: false,
            interfaces: Arc::new(RwLock::new(map)),
        }
    }
}

#[async_trait]
impl WireGuardEnginePort for MockWireGuardEngine {
    async fn configure_interface(
        &self,
        server: &Server,
        iface: &WireGuardInterface,
    ) -> Result<(), DomainError> {
        if self.should_fail {
            return Err(DomainError::ExecutionFailed {
                reason: format!("Failed to configure WireGuard on {}", server.host()),
                details: None,
            });
        }
        self.interfaces
            .write()
            .unwrap()
            .insert(iface.name().to_string(), iface.clone());
        Ok(())
    }

    async fn add_peer(
        &self,
        server: &Server,
        iface_name: &str,
        peer: &WireGuardPeer,
    ) -> Result<(), DomainError> {
        if self.should_fail {
            return Err(DomainError::ExecutionFailed {
                reason: format!("Failed to add WireGuard peer on {}", server.host()),
                details: None,
            });
        }
        let mut map = self.interfaces.write().unwrap();
        let iface = map
            .get_mut(iface_name)
            .ok_or_else(|| DomainError::NotFound {
                entity: "WireGuardInterface",
                id: iface_name.to_string(),
            })?;
        iface.add_peer(peer.clone());
        Ok(())
    }

    async fn remove_peer(
        &self,
        server: &Server,
        iface_name: &str,
        peer_pubkey: &str,
    ) -> Result<(), DomainError> {
        if self.should_fail {
            return Err(DomainError::ExecutionFailed {
                reason: format!("Failed to remove WireGuard peer on {}", server.host()),
                details: None,
            });
        }
        let mut map = self.interfaces.write().unwrap();
        let iface = map
            .get_mut(iface_name)
            .ok_or_else(|| DomainError::NotFound {
                entity: "WireGuardInterface",
                id: iface_name.to_string(),
            })?;
        iface.remove_peer(peer_pubkey);
        Ok(())
    }

    async fn get_device_status(
        &self,
        _server: &Server,
        iface_name: &str,
    ) -> Result<WireGuardDeviceStatus, DomainError> {
        let map = self.interfaces.read().unwrap();
        let iface = map.get(iface_name).ok_or_else(|| DomainError::NotFound {
            entity: "WireGuardInterface",
            id: iface_name.to_string(),
        })?;

        Ok(WireGuardDeviceStatus::new(
            iface.name(),
            iface.public_key(),
            iface.listen_port(),
            iface.peers().len(),
            5242880,  // 5 MB rx
            10485760, // 10 MB tx
            true,
        ))
    }

    async fn list_peers(
        &self,
        _server: &Server,
        iface_name: &str,
    ) -> Result<Vec<WireGuardPeer>, DomainError> {
        let map = self.interfaces.read().unwrap();
        let iface = map.get(iface_name).ok_or_else(|| DomainError::NotFound {
            entity: "WireGuardInterface",
            id: iface_name.to_string(),
        })?;
        Ok(iface.peers().to_vec())
    }

    async fn generate_keypair(&self) -> Result<(String, String), DomainError> {
        Ok((
            "4KV9m9pXQpI2WbL5Uv8Tz3RyK7N1F4J8S6H0A2C4E6g=".to_string(),
            "8N1F4J8S6H0A2C4E6g4KV9m9pXQpI2WbL5Uv8Tz3RyK=".to_string(),
        ))
    }
}

pub struct CliWireGuardAdapter {
    wg_bin: String,
}

impl Default for CliWireGuardAdapter {
    fn default() -> Self {
        Self {
            wg_bin: "wg".to_string(),
        }
    }
}

#[async_trait]
impl WireGuardEnginePort for CliWireGuardAdapter {
    async fn configure_interface(
        &self,
        _server: &Server,
        iface: &WireGuardInterface,
    ) -> Result<(), DomainError> {
        let config = iface.render_wg_quick_config();
        let tmp_path = format!("/tmp/{}.conf", iface.name());
        tokio::fs::write(&tmp_path, config).await.map_err(|e| {
            DomainError::ExecutionFailed {
                reason: format!("Failed to write WireGuard config file {}: {}", tmp_path, e),
                details: None,
            }
        })?;

        let output = Command::new("wg-quick")
            .arg("up")
            .arg(&tmp_path)
            .output()
            .await
            .map_err(|e| DomainError::ExecutionFailed {
                reason: format!("Failed to execute wg-quick up: {}", e),
                details: None,
            })?;

        if !output.status.success() {
            return Err(DomainError::ExecutionFailed {
                reason: "wg-quick up returned error".to_string(),
                details: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            });
        }
        Ok(())
    }

    async fn add_peer(
        &self,
        _server: &Server,
        iface_name: &str,
        peer: &WireGuardPeer,
    ) -> Result<(), DomainError> {
        let mut cmd = Command::new(&self.wg_bin);
        cmd.arg("set")
            .arg(iface_name)
            .arg("peer")
            .arg(&peer.public_key)
            .arg("allowed-ips")
            .arg(peer.allowed_ips.join(","));

        if let Some(ref ep) = peer.endpoint {
            cmd.arg("endpoint").arg(ep);
        }
        if let Some(keepalive) = peer.persistent_keepalive {
            cmd.arg("persistent-keepalive").arg(keepalive.to_string());
        }

        let output = cmd.output().await.map_err(|e| DomainError::ExecutionFailed {
            reason: format!("Failed to execute wg set peer: {}", e),
            details: None,
        })?;

        if !output.status.success() {
            return Err(DomainError::ExecutionFailed {
                reason: "wg set peer failed".to_string(),
                details: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            });
        }
        Ok(())
    }

    async fn remove_peer(
        &self,
        _server: &Server,
        iface_name: &str,
        peer_pubkey: &str,
    ) -> Result<(), DomainError> {
        let output = Command::new(&self.wg_bin)
            .arg("set")
            .arg(iface_name)
            .arg("peer")
            .arg(peer_pubkey)
            .arg("remove")
            .output()
            .await
            .map_err(|e| DomainError::ExecutionFailed {
                reason: format!("Failed to remove WireGuard peer: {}", e),
                details: None,
            })?;

        if !output.status.success() {
            return Err(DomainError::ExecutionFailed {
                reason: "wg set peer remove failed".to_string(),
                details: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            });
        }
        Ok(())
    }

    async fn get_device_status(
        &self,
        _server: &Server,
        iface_name: &str,
    ) -> Result<WireGuardDeviceStatus, DomainError> {
        let output = Command::new(&self.wg_bin)
            .arg("show")
            .arg(iface_name)
            .output()
            .await
            .map_err(|e| DomainError::ExecutionFailed {
                reason: format!("Failed to execute wg show {}: {}", iface_name, e),
                details: None,
            })?;

        if !output.status.success() {
            return Err(DomainError::NotFound {
                entity: "WireGuardInterface",
                id: iface_name.to_string(),
            });
        }

        Ok(WireGuardDeviceStatus::new(
            iface_name,
            "pubkey",
            51820,
            0,
            0,
            0,
            true,
        ))
    }

    async fn list_peers(
        &self,
        _server: &Server,
        _iface_name: &str,
    ) -> Result<Vec<WireGuardPeer>, DomainError> {
        Ok(vec![])
    }

    async fn generate_keypair(&self) -> Result<(String, String), DomainError> {
        let genkey_out = Command::new(&self.wg_bin)
            .arg("genkey")
            .output()
            .await
            .map_err(|e| DomainError::ExecutionFailed {
                reason: format!("Failed to spawn wg genkey: {}", e),
                details: None,
            })?;

        let privkey = String::from_utf8_lossy(&genkey_out.stdout).trim().to_string();

        use tokio::io::AsyncWriteExt;
        let mut pubkey_cmd = Command::new(&self.wg_bin);
        pubkey_cmd
            .arg("pubkey")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped());

        let mut child = pubkey_cmd.spawn().map_err(|e| DomainError::ExecutionFailed {
            reason: format!("Failed to spawn wg pubkey: {}", e),
            details: None,
        })?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(privkey.as_bytes()).await;
        }

        let pubkey_out = child.wait_with_output().await.map_err(|e| DomainError::ExecutionFailed {
            reason: format!("Failed to wait for wg pubkey: {}", e),
            details: None,
        })?;

        let pubkey = String::from_utf8_lossy(&pubkey_out.stdout).trim().to_string();

        Ok((privkey, pubkey))
    }
}
