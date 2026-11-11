use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{errors::DomainError, orchestration::JobId};

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

impl std::fmt::Display for HostAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.port == 22 {
            write!(f, "{}", self.ip_or_host)
        } else {
            write!(f, "{}:{}", self.ip_or_host, self.port)
        }
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerMetrics {
    pub cpu_usage_pct: f32,
    pub ram_used_mb: u64,
    pub ram_total_mb: u64,
    pub disk_used_gb: f32,
    pub disk_total_gb: f32,
    pub uptime_seconds: u64,
    pub load_avg: [f32; 3],
}

impl ServerMetrics {
    pub fn new(
        cpu_usage_pct: f32,
        ram_used_mb: u64,
        ram_total_mb: u64,
        disk_used_gb: f32,
        disk_total_gb: f32,
        uptime_seconds: u64,
        load_avg: [f32; 3],
    ) -> Result<Self, DomainError> {
        if !(0.0..=100.0).contains(&cpu_usage_pct) {
            return Err(DomainError::ValidationError(
                "CPU usage percentage must be between 0.0 and 100.0".to_string(),
            ));
        }
        if ram_used_mb > ram_total_mb {
            return Err(DomainError::ValidationError(
                "RAM used cannot exceed total RAM".to_string(),
            ));
        }
        if disk_used_gb > disk_total_gb {
            return Err(DomainError::ValidationError(
                "Disk used cannot exceed total Disk".to_string(),
            ));
        }

        Ok(Self {
            cpu_usage_pct,
            ram_used_mb,
            ram_total_mb,
            disk_used_gb,
            disk_total_gb,
            uptime_seconds,
            load_avg,
        })
    }

    pub fn ram_percentage(&self) -> f32 {
        if self.ram_total_mb == 0 {
            return 0.0;
        }
        (self.ram_used_mb as f32 / self.ram_total_mb as f32) * 100.0
    }

    pub fn disk_percentage(&self) -> f32 {
        if self.disk_total_gb <= 0.0 {
            return 0.0;
        }
        (self.disk_used_gb / self.disk_total_gb) * 100.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceAction {
    Start,
    Stop,
    Restart,
    Reload,
    Status,
}

impl ServiceAction {
    pub fn as_systemd_command(&self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
            Self::Reload => "reload",
            Self::Status => "status",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceStatus {
    Active,
    Inactive,
    Failed,
    Unknown,
}

impl ServiceStatus {
    pub fn is_healthy(&self) -> bool {
        matches!(self, Self::Active)
    }
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
    locked_by_job: Option<JobId>,
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
            locked_by_job: None,
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

    pub fn is_locked(&self) -> bool {
        self.locked_by_job.is_some()
    }

    pub fn locked_by_job(&self) -> Option<JobId> {
        self.locked_by_job
    }

    pub fn acquire_lock(&mut self, job_id: JobId) -> Result<(), DomainError> {
        if let Some(existing) = self.locked_by_job {
            return Err(DomainError::Conflict(format!(
                "Server is already locked by job {}",
                existing
            )));
        }
        self.locked_by_job = Some(job_id);
        Ok(())
    }

    pub fn release_lock(&mut self, job_id: JobId) -> Result<(), DomainError> {
        match self.locked_by_job {
            Some(existing) if existing == job_id => {
                self.locked_by_job = None;
                Ok(())
            }
            Some(existing) => Err(DomainError::Conflict(format!(
                "Cannot release lock: server was locked by job {}, not {}",
                existing, job_id
            ))),
            None => Ok(()),
        }
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
