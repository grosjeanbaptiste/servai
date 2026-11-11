use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnsibleTaskEvent {
    pub task_name: String,
    pub host: String,
    pub status: String,
    pub stdout: Option<String>,
}

impl AnsibleTaskEvent {
    pub fn new(
        task_name: impl Into<String>,
        host: impl Into<String>,
        status: impl Into<String>,
        stdout: Option<String>,
    ) -> Self {
        Self {
            task_name: task_name.into(),
            host: host.into(),
            status: status.into(),
            stdout,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnsibleRunResult {
    ok_count: u32,
    changed_count: u32,
    failed_count: u32,
    unreachable_count: u32,
    task_events: Vec<AnsibleTaskEvent>,
}

impl AnsibleRunResult {
    pub fn new(
        ok_count: u32,
        changed_count: u32,
        failed_count: u32,
        unreachable_count: u32,
        task_events: Vec<AnsibleTaskEvent>,
    ) -> Self {
        Self {
            ok_count,
            changed_count,
            failed_count,
            unreachable_count,
            task_events,
        }
    }

    pub fn ok_count(&self) -> u32 {
        self.ok_count
    }

    pub fn changed_count(&self) -> u32 {
        self.changed_count
    }

    pub fn failed_count(&self) -> u32 {
        self.failed_count
    }

    pub fn unreachable_count(&self) -> u32 {
        self.unreachable_count
    }

    pub fn task_events(&self) -> &[AnsibleTaskEvent] {
        &self.task_events
    }

    pub fn is_successful(&self) -> bool {
        self.failed_count == 0 && self.unreachable_count == 0
    }

    pub fn has_unreachable(&self) -> bool {
        self.unreachable_count > 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnsibleFacts {
    pub distribution: String,
    pub distribution_version: String,
    pub kernel: String,
    pub python_version: String,
    pub interfaces: Vec<String>,
    pub memtotal_mb: u64,
}

impl AnsibleFacts {
    pub fn new(
        distribution: impl Into<String>,
        distribution_version: impl Into<String>,
        kernel: impl Into<String>,
        python_version: impl Into<String>,
        interfaces: Vec<String>,
        memtotal_mb: u64,
    ) -> Self {
        Self {
            distribution: distribution.into(),
            distribution_version: distribution_version.into(),
            kernel: kernel.into(),
            python_version: python_version.into(),
            interfaces,
            memtotal_mb,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnsibleModuleResult {
    pub module: String,
    pub changed: bool,
    pub output: String,
    pub rc: i32,
}

impl AnsibleModuleResult {
    pub fn new(
        module: impl Into<String>,
        changed: bool,
        output: impl Into<String>,
        rc: i32,
    ) -> Self {
        Self {
            module: module.into(),
            changed,
            output: output.into(),
            rc,
        }
    }

    pub fn is_success(&self) -> bool {
        self.rc == 0
    }
}
