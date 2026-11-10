use async_trait::async_trait;
use std::sync::Arc;

use crate::domain::{
    errors::DomainError,
    server::{HostAddress, Server, ServerId, ServerName, ServerStatus},
};
pub use crate::ports::inbound::server_admin::RegisterServerCommand;
use crate::ports::{
    inbound::server_admin::ServerAdminPort,
    outbound::{
        server_executor::{CommandResult, ServerExecutorPort},
        server_store::ServerStorePort,
    },
};

pub struct ServerService {
    server_store: Arc<dyn ServerStorePort>,
    executor: Arc<dyn ServerExecutorPort>,
}

impl ServerService {
    pub fn new(
        server_store: Arc<dyn ServerStorePort>,
        executor: Arc<dyn ServerExecutorPort>,
    ) -> Self {
        Self {
            server_store,
            executor,
        }
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
}
