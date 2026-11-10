use async_trait::async_trait;
use crate::domain::{
    errors::DomainError,
    orchestration::OrchestrationStep,
    repository::ManagedRepository,
    server::{Server, ServerSpec},
};
use crate::ports::outbound::{
    git_provider::GitProviderPort,
    server_executor::{CommandResult, ServerExecutorPort},
};

#[derive(Clone)]
pub struct MockServerExecutor {
    pub should_fail_ping: bool,
    pub command_output: String,
    pub should_fail_command: bool,
}

impl Default for MockServerExecutor {
    fn default() -> Self {
        Self {
            should_fail_ping: false,
            command_output: "Success".to_string(),
            should_fail_command: false,
        }
    }
}

#[async_trait]
impl ServerExecutorPort for MockServerExecutor {
    async fn ping(&self, _server: &Server) -> Result<bool, DomainError> {
        if self.should_fail_ping {
            Ok(false)
        } else {
            Ok(true)
        }
    }

    async fn execute_command(&self, _server: &Server, _command: &str) -> Result<CommandResult, DomainError> {
        if self.should_fail_command {
            Err(DomainError::ExecutionFailed {
                reason: "Command returned exit code 1".to_string(),
                details: Some("Mock error output".to_string()),
            })
        } else {
            Ok(CommandResult {
                exit_code: 0,
                stdout: self.command_output.clone(),
                stderr: String::new(),
            })
        }
    }

    async fn inspect_system(&self, _server: &Server) -> Result<ServerSpec, DomainError> {
        Ok(ServerSpec {
            os: "Debian 12".to_string(),
            arch: "aarch64".to_string(),
            cpu_cores: 4,
            ram_mb: 16384,
        })
    }
}

#[derive(Clone)]
pub struct MockGitProvider {
    pub commit_hash: String,
    pub should_fail_sync: bool,
}

impl Default for MockGitProvider {
    fn default() -> Self {
        Self {
            commit_hash: "abcdef1234567890".to_string(),
            should_fail_sync: false,
        }
    }
}

#[async_trait]
impl GitProviderPort for MockGitProvider {
    async fn sync_repository(&self, _repo: &ManagedRepository) -> Result<String, DomainError> {
        if self.should_fail_sync {
            Err(DomainError::VcsError("Failed to clone/pull repository".to_string()))
        } else {
            Ok(self.commit_hash.clone())
        }
    }

    async fn resolve_plan(&self, _repo: &ManagedRepository) -> Result<Vec<OrchestrationStep>, DomainError> {
        Ok(vec![
            OrchestrationStep::new("git_pull", "Pull latest changes"),
            OrchestrationStep::new("deploy_service", "Apply configurations"),
        ])
    }
}
