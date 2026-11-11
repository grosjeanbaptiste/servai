use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use tokio::process::Command;

use crate::domain::{
    engines::opentofu::{TofuApplyResult, TofuPlanResult, TofuPlanSummary, TofuState},
    errors::DomainError,
};
use crate::ports::outbound::opentofu_engine::OpenTofuEnginePort;

#[derive(Clone)]
pub struct MockOpenTofuEngine {
    pub plan_summary: TofuPlanSummary,
    pub apply_outputs: HashMap<String, String>,
    pub should_fail: bool,
}

impl Default for MockOpenTofuEngine {
    fn default() -> Self {
        let mut outputs = HashMap::new();
        outputs.insert("cluster_endpoint".to_string(), "https://10.0.0.10:6443".to_string());
        outputs.insert("primary_ip".to_string(), "192.168.10.50".to_string());

        Self {
            plan_summary: TofuPlanSummary::new(2, 0, 0),
            apply_outputs: outputs,
            should_fail: false,
        }
    }
}

#[async_trait]
impl OpenTofuEnginePort for MockOpenTofuEngine {
    async fn init(&self, _repo_path: &Path) -> Result<(), DomainError> {
        if self.should_fail {
            Err(DomainError::ExecutionFailed {
                reason: "OpenTofu init failed: plugin download error".to_string(),
                details: None,
            })
        } else {
            Ok(())
        }
    }

    async fn plan(&self, _repo_path: &Path, _vars: HashMap<String, String>) -> Result<TofuPlanResult, DomainError> {
        if self.should_fail {
            Err(DomainError::ExecutionFailed {
                reason: "OpenTofu plan failed".to_string(),
                details: None,
            })
        } else {
            Ok(TofuPlanResult::new(
                self.plan_summary,
                vec!["server_instance.node[0]".to_string(), "server_instance.node[1]".to_string()],
            ))
        }
    }

    async fn apply(&self, _repo_path: &Path, _vars: HashMap<String, String>) -> Result<TofuApplyResult, DomainError> {
        if self.should_fail {
            Err(DomainError::ExecutionFailed {
                reason: "OpenTofu apply failed: API rate limit exceeded".to_string(),
                details: None,
            })
        } else {
            Ok(TofuApplyResult::new(
                self.plan_summary.to_add,
                self.plan_summary.to_change,
                self.plan_summary.to_destroy,
                self.apply_outputs.clone(),
                true,
            ))
        }
    }

    async fn destroy(&self, _repo_path: &Path, _vars: HashMap<String, String>) -> Result<TofuApplyResult, DomainError> {
        Ok(TofuApplyResult::new(0, 0, 2, HashMap::new(), true))
    }

    async fn get_state(&self, _repo_path: &Path) -> Result<TofuState, DomainError> {
        Ok(TofuState::new(
            1,
            "mock-lineage-123".to_string(),
            vec!["server_instance.node[0]".to_string(), "server_instance.node[1]".to_string()],
            self.apply_outputs.clone(),
        ))
    }
}

pub struct CliOpenTofuAdapter {
    bin_path: String,
}

impl Default for CliOpenTofuAdapter {
    fn default() -> Self {
        Self {
            bin_path: "tofu".to_string(),
        }
    }
}

#[async_trait]
impl OpenTofuEnginePort for CliOpenTofuAdapter {
    async fn init(&self, repo_path: &Path) -> Result<(), DomainError> {
        let output = Command::new(&self.bin_path)
            .current_dir(repo_path)
            .arg("init")
            .arg("-no-color")
            .output()
            .await
            .map_err(|e| DomainError::ExecutionFailed {
                reason: format!("Failed to spawn {}: {}", self.bin_path, e),
                details: None,
            })?;

        if !output.status.success() {
            return Err(DomainError::ExecutionFailed {
                reason: "OpenTofu init exited with error".to_string(),
                details: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            });
        }
        Ok(())
    }

    async fn plan(&self, repo_path: &Path, vars: HashMap<String, String>) -> Result<TofuPlanResult, DomainError> {
        let mut cmd = Command::new(&self.bin_path);
        cmd.current_dir(repo_path)
            .arg("plan")
            .arg("-no-color");

        for (k, v) in vars {
            cmd.arg("-var").arg(format!("{}={}", k, v));
        }

        let output = cmd.output().await.map_err(|e| DomainError::ExecutionFailed {
            reason: format!("Failed to spawn {}: {}", self.bin_path, e),
            details: None,
        })?;

        if !output.status.success() {
            return Err(DomainError::ExecutionFailed {
                reason: "OpenTofu plan failed".to_string(),
                details: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            });
        }

        Ok(TofuPlanResult::new(TofuPlanSummary::new(1, 0, 0), vec![]))
    }

    async fn apply(&self, repo_path: &Path, vars: HashMap<String, String>) -> Result<TofuApplyResult, DomainError> {
        let mut cmd = Command::new(&self.bin_path);
        cmd.current_dir(repo_path)
            .arg("apply")
            .arg("-auto-approve")
            .arg("-no-color");

        for (k, v) in vars {
            cmd.arg("-var").arg(format!("{}={}", k, v));
        }

        let output = cmd.output().await.map_err(|e| DomainError::ExecutionFailed {
            reason: format!("Failed to spawn {}: {}", self.bin_path, e),
            details: None,
        })?;

        if !output.status.success() {
            return Err(DomainError::ExecutionFailed {
                reason: "OpenTofu apply failed".to_string(),
                details: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            });
        }

        Ok(TofuApplyResult::new(1, 0, 0, HashMap::new(), true))
    }

    async fn destroy(&self, repo_path: &Path, vars: HashMap<String, String>) -> Result<TofuApplyResult, DomainError> {
        let mut cmd = Command::new(&self.bin_path);
        cmd.current_dir(repo_path)
            .arg("destroy")
            .arg("-auto-approve")
            .arg("-no-color");

        for (k, v) in vars {
            cmd.arg("-var").arg(format!("{}={}", k, v));
        }

        let output = cmd.output().await.map_err(|e| DomainError::ExecutionFailed {
            reason: format!("Failed to spawn {}: {}", self.bin_path, e),
            details: None,
        })?;

        if !output.status.success() {
            return Err(DomainError::ExecutionFailed {
                reason: "OpenTofu destroy failed".to_string(),
                details: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            });
        }

        Ok(TofuApplyResult::new(0, 0, 1, HashMap::new(), true))
    }

    async fn get_state(&self, repo_path: &Path) -> Result<TofuState, DomainError> {
        let output = Command::new(&self.bin_path)
            .current_dir(repo_path)
            .arg("show")
            .arg("-json")
            .output()
            .await
            .map_err(|e| DomainError::ExecutionFailed {
                reason: format!("Failed to spawn {}: {}", self.bin_path, e),
                details: None,
            })?;

        if !output.status.success() {
            return Err(DomainError::ExecutionFailed {
                reason: "OpenTofu state inspect failed".to_string(),
                details: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            });
        }

        Ok(TofuState::new(1, "tofu-state-live".to_string(), vec![], HashMap::new()))
    }
}
