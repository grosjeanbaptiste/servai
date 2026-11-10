use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::domain::{
    errors::DomainError,
    server::{Server, ServerMetrics, ServerSpec, ServiceAction, ServiceStatus},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[async_trait]
pub trait ServerExecutorPort: Send + Sync {
    async fn ping(&self, server: &Server) -> Result<bool, DomainError>;
    async fn execute_command(&self, server: &Server, command: &str) -> Result<CommandResult, DomainError>;
    async fn inspect_system(&self, server: &Server) -> Result<ServerSpec, DomainError>;
    async fn manage_system_service(
        &self,
        server: &Server,
        service: &str,
        action: ServiceAction,
    ) -> Result<ServiceStatus, DomainError>;
    async fn collect_metrics(&self, server: &Server) -> Result<ServerMetrics, DomainError>;
    async fn reboot(&self, server: &Server) -> Result<(), DomainError>;
}
