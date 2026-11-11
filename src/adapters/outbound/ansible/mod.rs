use async_trait::async_trait;
use std::collections::HashMap;
use tokio::process::Command;

use crate::domain::{
    engines::ansible::{AnsibleFacts, AnsibleModuleResult, AnsibleRunResult, AnsibleTaskEvent},
    errors::DomainError,
    server::Server,
};
use crate::ports::outbound::ansible_engine::AnsibleEnginePort;

#[derive(Clone)]
pub struct MockAnsibleEngine {
    pub should_fail: bool,
    pub run_result: AnsibleRunResult,
    pub module_result: AnsibleModuleResult,
    pub facts: AnsibleFacts,
}

impl Default for MockAnsibleEngine {
    fn default() -> Self {
        Self {
            should_fail: false,
            run_result: AnsibleRunResult::new(
                3,
                1,
                0,
                0,
                vec![
                    AnsibleTaskEvent::new("Gathering Facts", "server-node-1", "ok", None),
                    AnsibleTaskEvent::new("Install Nginx", "server-node-1", "changed", Some("apt install nginx done".into())),
                    AnsibleTaskEvent::new("Start Nginx Service", "server-node-1", "ok", None),
                ],
            ),
            module_result: AnsibleModuleResult::new("command", true, "output: service reloaded", 0),
            facts: AnsibleFacts::new(
                "Ubuntu",
                "22.04",
                "5.15.0-generic",
                "3.10.12",
                vec!["eth0".to_string(), "lo".to_string()],
                8192,
            ),
        }
    }
}

#[async_trait]
impl AnsibleEnginePort for MockAnsibleEngine {
    async fn run_playbook(
        &self,
        server: &Server,
        _playbook_name: &str,
        _extra_vars: HashMap<String, String>,
    ) -> Result<AnsibleRunResult, DomainError> {
        if self.should_fail {
            Err(DomainError::ExecutionFailed {
                reason: format!("Ansible playbook failed on host {}", server.host()),
                details: None,
            })
        } else {
            Ok(self.run_result.clone())
        }
    }

    async fn run_module(
        &self,
        server: &Server,
        module: &str,
        _args: &str,
    ) -> Result<AnsibleModuleResult, DomainError> {
        if self.should_fail {
            Err(DomainError::ExecutionFailed {
                reason: format!("Ansible module {} failed on {}", module, server.host()),
                details: None,
            })
        } else {
            Ok(self.module_result.clone())
        }
    }

    async fn gather_facts(&self, server: &Server) -> Result<AnsibleFacts, DomainError> {
        if self.should_fail {
            Err(DomainError::ExecutionFailed {
                reason: format!("Ansible fact gathering failed for {}", server.host()),
                details: None,
            })
        } else {
            Ok(self.facts.clone())
        }
    }
}

pub struct CliAnsibleAdapter {
    bin_path: String,
}

impl Default for CliAnsibleAdapter {
    fn default() -> Self {
        Self {
            bin_path: "ansible-playbook".to_string(),
        }
    }
}

#[async_trait]
impl AnsibleEnginePort for CliAnsibleAdapter {
    async fn run_playbook(
        &self,
        server: &Server,
        playbook_name: &str,
        extra_vars: HashMap<String, String>,
    ) -> Result<AnsibleRunResult, DomainError> {
        let mut cmd = Command::new(&self.bin_path);
        let inventory = format!("{},", server.host());
        cmd.arg("-i").arg(inventory)
           .arg(playbook_name);

        if !extra_vars.is_empty() {
            let vars_json = serde_json::to_string(&extra_vars).unwrap_or_default();
            cmd.arg("--extra-vars").arg(vars_json);
        }

        let output = cmd.output().await.map_err(|e| DomainError::ExecutionFailed {
            reason: format!("Failed to spawn {}: {}", self.bin_path, e),
            details: None,
        })?;

        if !output.status.success() {
            return Err(DomainError::ExecutionFailed {
                reason: "Ansible playbook execution returned non-zero code".to_string(),
                details: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            });
        }

        Ok(AnsibleRunResult::new(1, 1, 0, 0, vec![]))
    }

    async fn run_module(
        &self,
        server: &Server,
        module: &str,
        args: &str,
    ) -> Result<AnsibleModuleResult, DomainError> {
        let mut cmd = Command::new("ansible");
        let inventory = format!("{},", server.host());
        cmd.arg("all")
           .arg("-i").arg(inventory)
           .arg("-m").arg(module)
           .arg("-a").arg(args);

        let output = cmd.output().await.map_err(|e| DomainError::ExecutionFailed {
            reason: format!("Failed to spawn ansible: {}", e),
            details: None,
        })?;

        let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
        let rc = output.status.code().unwrap_or(-1);

        Ok(AnsibleModuleResult::new(
            module,
            stdout_str.contains("CHANGED"),
            stdout_str,
            rc,
        ))
    }

    async fn gather_facts(&self, server: &Server) -> Result<AnsibleFacts, DomainError> {
        let mut cmd = Command::new("ansible");
        let inventory = format!("{},", server.host());
        cmd.arg("all")
           .arg("-i").arg(inventory)
           .arg("-m").arg("setup");

        let output = cmd.output().await.map_err(|e| DomainError::ExecutionFailed {
            reason: format!("Failed to spawn ansible setup: {}", e),
            details: None,
        })?;

        if !output.status.success() {
            return Err(DomainError::ExecutionFailed {
                reason: "Ansible setup module failed".to_string(),
                details: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            });
        }

        Ok(AnsibleFacts::new("Linux", "Generic", "kernel", "3.10", vec!["eth0".into()], 4096))
    }
}
