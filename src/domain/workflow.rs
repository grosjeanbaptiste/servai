use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{
    errors::DomainError,
    orchestration::{ExecutionStatus, JobId, OrchestrationJob},
    server::ServerId,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepLogEntry {
    pub step_index: usize,
    pub name: String,
    pub status: ExecutionStatus,
    pub output: Option<String>,
    pub error: Option<String>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobExecutionLogs {
    pub job_id: JobId,
    pub status: ExecutionStatus,
    pub steps: Vec<StepLogEntry>,
    pub total_duration_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkflowId(Uuid);

impl WorkflowId {
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

impl Default for WorkflowId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for WorkflowId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowAction {
    ShellCommand { command: String },
    DnsRecordUpsert { domain: String, record_type: String, value: String },
    HttpHealthCheck { url: String, expected_status: u16 },
    ServiceAction { service: String, action: String },
    PackageInstall { package_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub name: String,
    pub description: String,
    pub action: WorkflowAction,
    pub rollback_command: Option<String>,
}

impl WorkflowStep {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        action: WorkflowAction,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            action,
            rollback_command: None,
        }
    }

    pub fn with_rollback(mut self, rollback_command: impl Into<String>) -> Self {
        self.rollback_command = Some(rollback_command.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub id: WorkflowId,
    pub name: String,
    pub description: String,
    pub steps: Vec<WorkflowStep>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl WorkflowDefinition {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        steps: Vec<WorkflowStep>,
    ) -> Result<Self, DomainError> {
        let n = name.into();
        let trimmed_name = n.trim();
        if trimmed_name.is_empty() {
            return Err(DomainError::ValidationError(
                "Workflow name cannot be empty".to_string(),
            ));
        }
        if steps.is_empty() {
            return Err(DomainError::ValidationError(
                "Workflow must contain at least one step".to_string(),
            ));
        }

        let now = Utc::now();
        Ok(Self {
            id: WorkflowId::new(),
            name: trimmed_name.to_string(),
            description: description.into(),
            steps,
            created_at: now,
            updated_at: now,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FleetRolloutStrategy {
    Parallel,
    Rolling { batch_size: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetRolloutCommand {
    pub target_tag: Option<String>,
    pub server_ids: Vec<ServerId>,
    pub strategy: FleetRolloutStrategy,
    pub action: WorkflowAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetJobResult {
    pub total_targets: usize,
    pub succeeded_count: usize,
    pub failed_count: usize,
    pub jobs: Vec<OrchestrationJob>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScheduleId(Uuid);

impl ScheduleId {
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

impl Default for ScheduleId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ScheduleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledJob {
    pub id: ScheduleId,
    pub name: String,
    pub cron_expression: String,
    pub server_id: ServerId,
    pub action: WorkflowAction,
    pub enabled: bool,
    pub last_run: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledJobCommand {
    pub name: String,
    pub cron_expression: String,
    pub server_id: ServerId,
    pub action: WorkflowAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesiredServerState {
    pub server_id: ServerId,
    pub required_packages: Vec<String>,
    pub bound_domain: Option<String>,
    pub wireguard_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconciliationResult {
    pub server_id: ServerId,
    pub drift_detected: bool,
    pub remediation_jobs: Vec<OrchestrationJob>,
}
