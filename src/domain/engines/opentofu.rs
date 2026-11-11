use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TofuPlanSummary {
    pub to_add: usize,
    pub to_change: usize,
    pub to_destroy: usize,
}

impl TofuPlanSummary {
    pub fn new(to_add: usize, to_change: usize, to_destroy: usize) -> Self {
        Self {
            to_add,
            to_change,
            to_destroy,
        }
    }

    pub fn has_changes(&self) -> bool {
        self.to_add > 0 || self.to_change > 0 || self.to_destroy > 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TofuPlanResult {
    summary: TofuPlanSummary,
    resources: Vec<String>,
}

impl TofuPlanResult {
    pub fn new(summary: TofuPlanSummary, resources: Vec<String>) -> Self {
        Self { summary, resources }
    }

    pub fn summary(&self) -> &TofuPlanSummary {
        &self.summary
    }

    pub fn resources(&self) -> &[String] {
        &self.resources
    }

    pub fn is_clean(&self) -> bool {
        !self.summary.has_changes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TofuApplyResult {
    resources_created: usize,
    resources_updated: usize,
    resources_deleted: usize,
    outputs: HashMap<String, String>,
    success: bool,
}

impl TofuApplyResult {
    pub fn new(
        resources_created: usize,
        resources_updated: usize,
        resources_deleted: usize,
        outputs: HashMap<String, String>,
        success: bool,
    ) -> Self {
        Self {
            resources_created,
            resources_updated,
            resources_deleted,
            outputs,
            success,
        }
    }

    pub fn resources_created(&self) -> usize {
        self.resources_created
    }

    pub fn resources_updated(&self) -> usize {
        self.resources_updated
    }

    pub fn resources_deleted(&self) -> usize {
        self.resources_deleted
    }

    pub fn outputs(&self) -> &HashMap<String, String> {
        &self.outputs
    }

    pub fn is_success(&self) -> bool {
        self.success
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TofuState {
    serial: u64,
    lineage: String,
    resources: Vec<String>,
    outputs: HashMap<String, String>,
}

impl TofuState {
    pub fn new(
        serial: u64,
        lineage: String,
        resources: Vec<String>,
        outputs: HashMap<String, String>,
    ) -> Self {
        Self {
            serial,
            lineage,
            resources,
            outputs,
        }
    }

    pub fn serial(&self) -> u64 {
        self.serial
    }

    pub fn lineage(&self) -> &str {
        &self.lineage
    }

    pub fn resources(&self) -> &[String] {
        &self.resources
    }

    pub fn outputs(&self) -> &HashMap<String, String> {
        &self.outputs
    }
}
