use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::domain::{
    errors::DomainError,
    orchestration::OrchestrationStep,
    repository::RepoId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StackId(Uuid);

impl StackId {
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

impl Default for StackId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for StackId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackRepoEntry {
    pub repo_id: RepoId,
    pub order: u32,
    pub step_name: String,
    pub command: String,
    pub rollback_command: Option<String>,
    pub overrides: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiRepoStack {
    id: StackId,
    name: String,
    description: String,
    entries: Vec<StackRepoEntry>,
}

impl MultiRepoStack {
    pub fn new(
        id: StackId,
        name: impl Into<String>,
        description: impl Into<String>,
        mut entries: Vec<StackRepoEntry>,
    ) -> Result<Self, DomainError> {
        let name_str = name.into();
        let trimmed_name = name_str.trim();
        if trimmed_name.is_empty() {
            return Err(DomainError::ValidationError(
                "Stack name cannot be empty".to_string(),
            ));
        }

        if entries.is_empty() {
            return Err(DomainError::ValidationError(
                "MultiRepoStack must have at least one repository entry".to_string(),
            ));
        }

        // Sort entries by order
        entries.sort_by_key(|e| e.order);

        Ok(Self {
            id,
            name: trimmed_name.to_string(),
            description: description.into(),
            entries,
        })
    }

    pub fn id(&self) -> StackId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn entries(&self) -> &[StackRepoEntry] {
        &self.entries
    }

    pub fn compile_to_orchestration_steps(&self) -> Vec<OrchestrationStep> {
        self.entries
            .iter()
            .map(|entry| {
                let mut step = OrchestrationStep::new(
                    entry.step_name.clone(),
                    format!("Execute stack step: {}", entry.command),
                );
                step.rollback_command = entry.rollback_command.clone();
                step
            })
            .collect()
    }
}
