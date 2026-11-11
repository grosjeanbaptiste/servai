use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::domain::errors::DomainError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossplaneCondition {
    pub type_name: String,
    pub status: String,
    pub reason: String,
    pub message: String,
}

impl CrossplaneCondition {
    pub fn new(
        type_name: impl Into<String>,
        status: impl Into<String>,
        reason: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            type_name: type_name.into(),
            status: status.into(),
            reason: reason.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossplaneClaim {
    api_version: String,
    kind: String,
    name: String,
    namespace: String,
    composition_ref: Option<String>,
    parameters: HashMap<String, String>,
}

impl CrossplaneClaim {
    pub fn new(
        api_version: impl Into<String>,
        kind: impl Into<String>,
        name: impl Into<String>,
        namespace: impl Into<String>,
        composition_ref: Option<String>,
        parameters: HashMap<String, String>,
    ) -> Result<Self, DomainError> {
        let name_str = name.into();
        let trimmed_name = name_str.trim();
        if trimmed_name.is_empty() {
            return Err(DomainError::ValidationError(
                "Crossplane claim name cannot be empty".to_string(),
            ));
        }

        let kind_str = kind.into();
        let trimmed_kind = kind_str.trim();
        if trimmed_kind.is_empty() {
            return Err(DomainError::ValidationError(
                "Crossplane claim kind cannot be empty".to_string(),
            ));
        }

        let ns_str = namespace.into();
        let trimmed_ns = if ns_str.trim().is_empty() {
            "default".to_string()
        } else {
            ns_str.trim().to_string()
        };

        Ok(Self {
            api_version: api_version.into(),
            kind: trimmed_kind.to_string(),
            name: trimmed_name.to_string(),
            namespace: trimmed_ns,
            composition_ref,
            parameters,
        })
    }

    pub fn api_version(&self) -> &str {
        &self.api_version
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn composition_ref(&self) -> Option<&str> {
        self.composition_ref.as_deref()
    }

    pub fn parameters(&self) -> &HashMap<String, String> {
        &self.parameters
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossplaneResource {
    pub kind: String,
    pub name: String,
    pub namespace: String,
    pub ready: bool,
    pub synced: bool,
    pub provider: String,
}

impl CrossplaneResource {
    pub fn new(
        kind: impl Into<String>,
        name: impl Into<String>,
        namespace: impl Into<String>,
        ready: bool,
        synced: bool,
        provider: impl Into<String>,
    ) -> Self {
        Self {
            kind: kind.into(),
            name: name.into(),
            namespace: namespace.into(),
            ready,
            synced,
            provider: provider.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossplaneReconcileResult {
    name: String,
    kind: String,
    ready: bool,
    synced: bool,
    conditions: Vec<CrossplaneCondition>,
    managed_resources: Vec<CrossplaneResource>,
}

impl CrossplaneReconcileResult {
    pub fn new(
        name: impl Into<String>,
        kind: impl Into<String>,
        ready: bool,
        synced: bool,
        conditions: Vec<CrossplaneCondition>,
        managed_resources: Vec<CrossplaneResource>,
    ) -> Self {
        Self {
            name: name.into(),
            kind: kind.into(),
            ready,
            synced,
            conditions,
            managed_resources,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn is_ready(&self) -> bool {
        self.ready
    }

    pub fn is_synced(&self) -> bool {
        self.synced
    }

    pub fn conditions(&self) -> &[CrossplaneCondition] {
        &self.conditions
    }

    pub fn managed_resources(&self) -> &[CrossplaneResource] {
        &self.managed_resources
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossplaneProviderStatus {
    pub name: String,
    pub installed: bool,
    pub healthy: bool,
    pub version: String,
}

impl CrossplaneProviderStatus {
    pub fn new(
        name: impl Into<String>,
        installed: bool,
        healthy: bool,
        version: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            installed,
            healthy,
            version: version.into(),
        }
    }
}
