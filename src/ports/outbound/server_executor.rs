use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::domain::{
    errors::DomainError,
    server::{Server, ServerSpec},
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
}
