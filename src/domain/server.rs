use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::errors::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ServerId(Uuid);

impl ServerId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for ServerId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ServerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerName(String);

impl ServerName {
    pub fn new(name: impl Into<String>) -> Result<Self, DomainError> {
        let s = name.into();
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(DomainError::ValidationError(
                "Server name cannot be empty".to_string(),
            ));
        }
        if trimmed.len() > 64 {
            return Err(DomainError::ValidationError(
                "Server name cannot exceed 64 characters".to_string(),
            ));
        }
        if !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return Err(DomainError::ValidationError(
                "Server name contains invalid characters (allowed: alphanumeric, -, _, .)".to_string(),
            ));
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostAddress {
    ip_or_host: String,
    port: u16,
}

impl HostAddress {
    pub fn new(ip_or_host: impl Into<String>, port: u16) -> Result<Self, DomainError> {
        let host = ip_or_host.into();
        let trimmed = host.trim();
        if trimmed.is_empty() {
            return Err(DomainError::ValidationError(
                "Host address cannot be empty".to_string(),
            ));
        }
        if port == 0 {
            return Err(DomainError::ValidationError(
                "Port must be greater than 0".to_string(),
            ));
        }
        // Basic hostname / IP character validation
        if !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == ':')
        {
            return Err(DomainError::ValidationError(
                "Host address contains invalid characters".to_string(),
            ));
        }
        Ok(Self {
            ip_or_host: trimmed.to_string(),
            port,
        })
    }

    pub fn ip(&self) -> &str {
        &self.ip_or_host
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SshCredentials {
    Agent,
    Password {
        username: String,
        password: String,
    },
    PrivateKey {
        username: String,
        key_content: String,
        passphrase: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerSpec {
    pub os: String,
    pub arch: String,
    pub cpu_cores: u32,
    pub ram_mb: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerStatus {
    Pending,
    Online,
    Offline,
    Maintenance,
    Degraded,
}

impl std::fmt::Display for ServerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "Pending"),
            Self::Online => write!(f, "Online"),
            Self::Offline => write!(f, "Offline"),
            Self::Maintenance => write!(f, "Maintenance"),
            Self::Degraded => write!(f, "Degraded"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Server {
    id: ServerId,
    name: ServerName,
    host: HostAddress,
    credentials: SshCredentials,
    spec: Option<ServerSpec>,
    status: ServerStatus,
    tags: Vec<String>,
}

impl Server {
    pub fn new(
        id: ServerId,
        name: ServerName,
        host: HostAddress,
        credentials: SshCredentials,
        spec: Option<ServerSpec>,
    ) -> Self {
        Self {
            id,
            name,
            host,
            credentials,
            spec,
            status: ServerStatus::Pending,
            tags: Vec::new(),
        }
    }

    pub fn id(&self) -> ServerId {
        self.id
    }

    pub fn name(&self) -> &ServerName {
        &self.name
    }

    pub fn host(&self) -> &HostAddress {
        &self.host
    }

    pub fn credentials(&self) -> &SshCredentials {
        &self.credentials
    }

    pub fn spec(&self) -> Option<&ServerSpec> {
        self.spec.as_ref()
    }

    pub fn status(&self) -> ServerStatus {
        self.status
    }

    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    pub fn add_tag(&mut self, tag: impl Into<String>) {
        let t = tag.into();
        if !self.tags.contains(&t) {
            self.tags.push(t);
        }
    }

    pub fn set_spec(&mut self, spec: ServerSpec) {
        self.spec = Some(spec);
    }

    pub fn transition_to(&mut self, new_status: ServerStatus) -> Result<(), DomainError> {
        if self.status == new_status {
            return Ok(());
        }

        let is_valid = match (self.status, new_status) {
            (ServerStatus::Pending, ServerStatus::Online | ServerStatus::Offline) => true,
            (
                ServerStatus::Online,
                ServerStatus::Maintenance | ServerStatus::Degraded | ServerStatus::Offline,
            ) => true,
            (ServerStatus::Maintenance, ServerStatus::Online | ServerStatus::Offline) => true,
            (
                ServerStatus::Degraded,
                ServerStatus::Online | ServerStatus::Maintenance | ServerStatus::Offline,
            ) => true,
            (
                ServerStatus::Offline,
                ServerStatus::Online | ServerStatus::Maintenance | ServerStatus::Pending,
            ) => true,
            _ => false,
        };

        if !is_valid {
            return Err(DomainError::InvalidStateTransition {
                from: self.status.to_string(),
                to: new_status.to_string(),
            });
        }

        self.status = new_status;
        Ok(())
    }
}
