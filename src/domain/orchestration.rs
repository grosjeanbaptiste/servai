use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{
    errors::DomainError,
    repository::RepoId,
    server::ServerId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(Uuid);

impl JobId {
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

impl Default for JobId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchestrationStep {
    pub name: String,
    pub description: String,
}

impl OrchestrationStep {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepExecution {
    pub step: OrchestrationStep,
    pub status: ExecutionStatus,
    pub output: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchestrationJob {
    id: JobId,
    server_id: ServerId,
    repo_id: RepoId,
    steps: Vec<StepExecution>,
    status: ExecutionStatus,
    current_step_index: usize,
    error_details: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl OrchestrationJob {
    pub fn new(
        id: JobId,
        server_id: ServerId,
        repo_id: RepoId,
        steps: Vec<OrchestrationStep>,
    ) -> Result<Self, DomainError> {
        if steps.is_empty() {
            return Err(DomainError::ValidationError(
                "An orchestration job must have at least one step".to_string(),
            ));
        }

        let step_executions = steps
            .into_iter()
            .map(|step| StepExecution {
                step,
                status: ExecutionStatus::Pending,
                output: None,
                error: None,
            })
            .collect();

        let now = Utc::now();
        Ok(Self {
            id,
            server_id,
            repo_id,
            steps: step_executions,
            status: ExecutionStatus::Pending,
            current_step_index: 0,
            error_details: None,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn id(&self) -> JobId {
        self.id
    }

    pub fn server_id(&self) -> ServerId {
        self.server_id
    }

    pub fn repo_id(&self) -> RepoId {
        self.repo_id
    }

    pub fn status(&self) -> ExecutionStatus {
        self.status
    }

    pub fn current_step_index(&self) -> usize {
        self.current_step_index
    }

    pub fn steps(&self) -> &[StepExecution] {
        &self.steps
    }

    pub fn error_details(&self) -> Option<&str> {
        self.error_details.as_deref()
    }

    pub fn start(&mut self) -> Result<(), DomainError> {
        if self.status != ExecutionStatus::Pending {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "Running".to_string(),
            });
        }
        self.status = ExecutionStatus::Running;
        self.updated_at = Utc::now();
        if let Some(first_step) = self.steps.get_mut(0) {
            first_step.status = ExecutionStatus::Running;
        }
        Ok(())
    }

    pub fn record_step_success(
        &mut self,
        step_idx: usize,
        output: impl Into<String>,
    ) -> Result<(), DomainError> {
        if self.status != ExecutionStatus::Running {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "StepSuccess".to_string(),
            });
        }
        if step_idx != self.current_step_index {
            return Err(DomainError::ValidationError(format!(
                "Invalid step index: expected {}, got {}",
                self.current_step_index, step_idx
            )));
        }

        let out = output.into();
        self.steps[step_idx].status = ExecutionStatus::Succeeded;
        self.steps[step_idx].output = Some(out);
        self.updated_at = Utc::now();

        if self.current_step_index + 1 < self.steps.len() {
            self.current_step_index += 1;
            self.steps[self.current_step_index].status = ExecutionStatus::Running;
        } else {
            self.status = ExecutionStatus::Succeeded;
        }

        Ok(())
    }

    pub fn record_step_failure(
        &mut self,
        step_idx: usize,
        reason: impl Into<String>,
        details: impl Into<String>,
    ) -> Result<(), DomainError> {
        if self.status != ExecutionStatus::Running {
            return Err(DomainError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "StepFailure".to_string(),
            });
        }
        if step_idx >= self.steps.len() {
            return Err(DomainError::ValidationError(format!(
                "Step index {} out of range",
                step_idx
            )));
        }

        let r = reason.into();
        let d = details.into();
        self.steps[step_idx].status = ExecutionStatus::Failed;
        self.steps[step_idx].error = Some(format!("{}: {}", r, d));
        self.error_details = Some(format!("{}: {}", r, d));
        self.status = ExecutionStatus::Failed;
        self.updated_at = Utc::now();

        Ok(())
    }
}
