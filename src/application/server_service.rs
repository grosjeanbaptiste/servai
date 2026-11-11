use async_trait::async_trait;
use std::sync::Arc;

pub use crate::ports::inbound::server_admin::RegisterServerCommand;
use crate::adapters::outbound::{
    ansible::MockAnsibleEngine,
    wireguard::MockWireGuardEngine,
};
use crate::domain::{
    engines::{
        ansible::{AnsibleFacts, AnsibleModuleResult},
        wireguard::{WireGuardDeviceStatus, WireGuardInterface, WireGuardPeer},
    },
    errors::DomainError,
    server::{HostAddress, Server, ServerId, ServerMetrics, ServerName, ServerStatus, ServiceAction, ServiceStatus},
};
use crate::ports::{
    inbound::server_admin::ServerAdminPort,
    outbound::{
        ansible_engine::AnsibleEnginePort,
        server_executor::{CommandResult, ServerExecutorPort},
        server_store::ServerStorePort,
        wireguard_engine::WireGuardEnginePort,
    },
};

pub struct ServerService {
    server_store: Arc<dyn ServerStorePort>,
    executor: Arc<dyn ServerExecutorPort>,
    ansible_engine: Arc<dyn AnsibleEnginePort>,
    wireguard_engine: Arc<dyn WireGuardEnginePort>,
}

impl ServerService {
    pub fn new(
        server_store: Arc<dyn ServerStorePort>,
        executor: Arc<dyn ServerExecutorPort>,
    ) -> Self {
        Self::new_with_engines(
            server_store,
            executor,
            Arc::new(MockAnsibleEngine::default()),
            Arc::new(MockWireGuardEngine::default()),
        )
    }

    pub fn new_with_ansible(
        server_store: Arc<dyn ServerStorePort>,
        executor: Arc<dyn ServerExecutorPort>,
        ansible_engine: Arc<dyn AnsibleEnginePort>,
    ) -> Self {
        Self::new_with_engines(
            server_store,
            executor,
            ansible_engine,
            Arc::new(MockWireGuardEngine::default()),
        )
    }

    pub fn new_with_engines(
        server_store: Arc<dyn ServerStorePort>,
        executor: Arc<dyn ServerExecutorPort>,
        ansible_engine: Arc<dyn AnsibleEnginePort>,
        wireguard_engine: Arc<dyn WireGuardEnginePort>,
    ) -> Self {
        Self {
            server_store,
            executor,
            ansible_engine,
            wireguard_engine,
        }
    }

    pub fn with_wireguard_engine(mut self, engine: Arc<dyn WireGuardEnginePort>) -> Self {
        self.wireguard_engine = engine;
        self
    }
}

#[async_trait]
impl ServerAdminPort for ServerService {
    async fn register_server(&self, cmd: RegisterServerCommand) -> Result<Server, DomainError> {
        let name = ServerName::new(cmd.name)?;
        let host = HostAddress::new(cmd.ip, cmd.port)?;
        let id = ServerId::new();

        let mut server = Server::new(id, name, host, cmd.credentials, None);
        for tag in cmd.tags {
            server.add_tag(tag);
        }

        self.server_store.save(server.clone()).await?;
        Ok(server)
    }

    async fn get_server(&self, id: &ServerId) -> Result<Server, DomainError> {
        self.server_store
            .find_by_id(id)
            .await?
            .ok_or_else(|| DomainError::NotFound {
                entity: "Server",
                id: id.to_string(),
            })
    }

    async fn list_servers(&self) -> Result<Vec<Server>, DomainError> {
        self.server_store.list().await
    }

    async fn health_check(&self, id: &ServerId) -> Result<ServerStatus, DomainError> {
        let mut server = self.get_server(id).await?;

        let is_reachable = self.executor.ping(&server).await?;
        let new_status = if is_reachable {
            if let Ok(spec) = self.executor.inspect_system(&server).await {
                server.set_spec(spec);
            }
            ServerStatus::Online
        } else {
            ServerStatus::Offline
        };

        server.transition_to(new_status)?;
        self.server_store.save(server.clone()).await?;

        Ok(new_status)
    }

    async fn set_maintenance(&self, id: &ServerId, enable: bool) -> Result<Server, DomainError> {
        let mut server = self.get_server(id).await?;
        let target_status = if enable {
            ServerStatus::Maintenance
        } else {
            ServerStatus::Online
        };
        server.transition_to(target_status)?;
        self.server_store.save(server.clone()).await?;
        Ok(server)
    }

    async fn execute_on_server(
        &self,
        id: &ServerId,
        command: &str,
    ) -> Result<CommandResult, DomainError> {
        let server = self.get_server(id).await?;
        if server.status() != ServerStatus::Online {
            return Err(DomainError::Conflict(format!(
                "Cannot execute command on server in status {:?}",
                server.status()
            )));
        }

        self.executor.execute_command(&server, command).await
    }

    async fn manage_service(
        &self,
        id: &ServerId,
        service: &str,
        action: ServiceAction,
    ) -> Result<ServiceStatus, DomainError> {
        let server = self.get_server(id).await?;
        if server.status() != ServerStatus::Online {
            return Err(DomainError::Conflict(format!(
                "Cannot manage service on server with status {:?}",
                server.status()
            )));
        }

        self.executor.manage_system_service(&server, service, action).await
    }

    async fn get_metrics(&self, id: &ServerId) -> Result<ServerMetrics, DomainError> {
        let server = self.get_server(id).await?;
        if server.status() != ServerStatus::Online {
            return Err(DomainError::Conflict(format!(
                "Cannot collect metrics on server with status {:?}",
                server.status()
            )));
        }

        self.executor.collect_metrics(&server).await
    }

    async fn reboot_server(&self, id: &ServerId) -> Result<(), DomainError> {
        let mut server = self.get_server(id).await?;
        self.executor.reboot(&server).await?;
        server.transition_to(ServerStatus::Maintenance)?;
        self.server_store.save(server).await?;
        Ok(())
    }

    async fn run_ansible_module(
        &self,
        id: &ServerId,
        module: &str,
        args: &str,
    ) -> Result<AnsibleModuleResult, DomainError> {
        let server = self.get_server(id).await?;
        if server.status() == ServerStatus::Offline {
            return Err(DomainError::Conflict(format!(
                "Cannot run ansible module on offline server {}",
                id
            )));
        }
        self.ansible_engine.run_module(&server, module, args).await
    }

    async fn gather_ansible_facts(&self, id: &ServerId) -> Result<AnsibleFacts, DomainError> {
        let server = self.get_server(id).await?;
        if server.status() == ServerStatus::Offline {
            return Err(DomainError::Conflict(format!(
                "Cannot gather ansible facts on offline server {}",
                id
            )));
        }
        self.ansible_engine.gather_facts(&server).await
    }

    async fn configure_wireguard(
        &self,
        id: &ServerId,
        iface: WireGuardInterface,
    ) -> Result<(), DomainError> {
        let server = self.get_server(id).await?;
        if server.status() == ServerStatus::Offline {
            return Err(DomainError::Conflict(format!(
                "Cannot configure WireGuard on offline server {}",
                id
            )));
        }
        self.wireguard_engine.configure_interface(&server, &iface).await
    }

    async fn add_wireguard_peer(
        &self,
        id: &ServerId,
        iface_name: &str,
        peer: WireGuardPeer,
    ) -> Result<(), DomainError> {
        let server = self.get_server(id).await?;
        if server.status() == ServerStatus::Offline {
            return Err(DomainError::Conflict(format!(
                "Cannot modify WireGuard peers on offline server {}",
                id
            )));
        }
        self.wireguard_engine.add_peer(&server, iface_name, &peer).await
    }

    async fn remove_wireguard_peer(
        &self,
        id: &ServerId,
        iface_name: &str,
        peer_pubkey: &str,
    ) -> Result<(), DomainError> {
        let server = self.get_server(id).await?;
        if server.status() == ServerStatus::Offline {
            return Err(DomainError::Conflict(format!(
                "Cannot remove WireGuard peers on offline server {}",
                id
            )));
        }
        self.wireguard_engine.remove_peer(&server, iface_name, peer_pubkey).await
    }

    async fn get_wireguard_status(
        &self,
        id: &ServerId,
        iface_name: &str,
    ) -> Result<WireGuardDeviceStatus, DomainError> {
        let server = self.get_server(id).await?;
        if server.status() == ServerStatus::Offline {
            return Err(DomainError::Conflict(format!(
                "Cannot query WireGuard status on offline server {}",
                id
            )));
        }
        self.wireguard_engine.get_device_status(&server, iface_name).await
    }

    async fn list_wireguard_peers(
        &self,
        id: &ServerId,
        iface_name: &str,
    ) -> Result<Vec<WireGuardPeer>, DomainError> {
        let server = self.get_server(id).await?;
        if server.status() == ServerStatus::Offline {
            return Err(DomainError::Conflict(format!(
                "Cannot list WireGuard peers on offline server {}",
                id
            )));
        }
        self.wireguard_engine.list_peers(&server, iface_name).await
    }

    async fn generate_wireguard_keypair(&self) -> Result<(String, String), DomainError> {
        self.wireguard_engine.generate_keypair().await
    }
}
