use async_trait::async_trait;
use std::path::Path;
use tokio::process::Command;

use crate::domain::{
    engines::crossplane::{
        CrossplaneClaim, CrossplaneCondition, CrossplaneProviderStatus, CrossplaneReconcileResult,
        CrossplaneResource,
    },
    errors::DomainError,
};
use crate::ports::outbound::crossplane_engine::CrossplaneEnginePort;

#[derive(Clone)]
pub struct MockCrossplaneEngine {
    pub should_fail: bool,
    pub is_ready: bool,
    pub is_synced: bool,
    pub providers: Vec<CrossplaneProviderStatus>,
    pub managed: Vec<CrossplaneResource>,
}

impl Default for MockCrossplaneEngine {
    fn default() -> Self {
        Self {
            should_fail: false,
            is_ready: true,
            is_synced: true,
            providers: vec![
                CrossplaneProviderStatus::new("provider-aws", true, true, "v0.47.0"),
                CrossplaneProviderStatus::new("provider-kubernetes", true, true, "v0.11.0"),
                CrossplaneProviderStatus::new("provider-helm", true, true, "v0.18.0"),
            ],
            managed: vec![
                CrossplaneResource::new(
                    "Instance",
                    "postgres-db-inst",
                    "default",
                    true,
                    true,
                    "provider-aws",
                ),
                CrossplaneResource::new(
                    "SecurityGroup",
                    "postgres-sg",
                    "default",
                    true,
                    true,
                    "provider-aws",
                ),
            ],
        }
    }
}

#[async_trait]
impl CrossplaneEnginePort for MockCrossplaneEngine {
    async fn apply_claim(
        &self,
        claim: &CrossplaneClaim,
    ) -> Result<CrossplaneReconcileResult, DomainError> {
        if self.should_fail {
            return Err(DomainError::ExecutionFailed {
                reason: format!("Failed to apply Crossplane claim: {}", claim.name()),
                details: None,
            });
        }

        let conditions = vec![
            CrossplaneCondition::new("Synced", "True", "ReconcileSuccess", "Resource synced successfully"),
            CrossplaneCondition::new("Ready", "True", "Available", "Composite resource is ready"),
        ];

        Ok(CrossplaneReconcileResult::new(
            claim.name(),
            claim.kind(),
            self.is_ready,
            self.is_synced,
            conditions,
            self.managed.clone(),
        ))
    }

    async fn get_claim_status(
        &self,
        namespace: &str,
        name: &str,
    ) -> Result<CrossplaneReconcileResult, DomainError> {
        if self.should_fail {
            return Err(DomainError::NotFound {
                entity: "CrossplaneClaim",
                id: format!("{}/{}", namespace, name),
            });
        }

        let conditions = vec![
            CrossplaneCondition::new("Synced", "True", "ReconcileSuccess", "Resource synced successfully"),
            CrossplaneCondition::new("Ready", "True", "Available", "Composite resource is ready"),
        ];

        Ok(CrossplaneReconcileResult::new(
            name,
            "CompositeClaim",
            self.is_ready,
            self.is_synced,
            conditions,
            self.managed.clone(),
        ))
    }

    async fn delete_claim(&self, _namespace: &str, _name: &str) -> Result<(), DomainError> {
        if self.should_fail {
            Err(DomainError::ExecutionFailed {
                reason: "Failed to delete claim: resource locked".to_string(),
                details: None,
            })
        } else {
            Ok(())
        }
    }

    async fn list_managed_resources(
        &self,
        _namespace: Option<&str>,
    ) -> Result<Vec<CrossplaneResource>, DomainError> {
        Ok(self.managed.clone())
    }

    async fn list_providers(&self) -> Result<Vec<CrossplaneProviderStatus>, DomainError> {
        Ok(self.providers.clone())
    }

    async fn render_composition(
        &self,
        _claim_path: &Path,
        _composition_path: &Path,
    ) -> Result<Vec<CrossplaneResource>, DomainError> {
        Ok(self.managed.clone())
    }
}

pub struct CliCrossplaneAdapter {
    kubectl_bin: String,
    crossplane_bin: String,
}

impl Default for CliCrossplaneAdapter {
    fn default() -> Self {
        Self {
            kubectl_bin: "kubectl".to_string(),
            crossplane_bin: "crossplane".to_string(),
        }
    }
}

#[async_trait]
impl CrossplaneEnginePort for CliCrossplaneAdapter {
    async fn apply_claim(
        &self,
        claim: &CrossplaneClaim,
    ) -> Result<CrossplaneReconcileResult, DomainError> {
        let manifest = format!(
            "apiVersion: {}\nkind: {}\nmetadata:\n  name: {}\n  namespace: {}\n",
            claim.api_version(),
            claim.kind(),
            claim.name(),
            claim.namespace()
        );

        let mut cmd = Command::new(&self.kubectl_bin);
        cmd.arg("apply").arg("-f").arg("-");
        
        // Write manifest to stdin
        use tokio::io::AsyncWriteExt;
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| DomainError::ExecutionFailed {
            reason: format!("Failed to spawn kubectl apply: {}", e),
            details: None,
        })?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(manifest.as_bytes()).await;
        }

        let output = child.wait_with_output().await.map_err(|e| DomainError::ExecutionFailed {
            reason: format!("Failed to wait for kubectl apply: {}", e),
            details: None,
        })?;

        if !output.status.success() {
            return Err(DomainError::ExecutionFailed {
                reason: "kubectl apply failed".to_string(),
                details: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            });
        }

        Ok(CrossplaneReconcileResult::new(
            claim.name(),
            claim.kind(),
            true,
            true,
            vec![],
            vec![],
        ))
    }

    async fn get_claim_status(
        &self,
        namespace: &str,
        name: &str,
    ) -> Result<CrossplaneReconcileResult, DomainError> {
        let output = Command::new(&self.kubectl_bin)
            .arg("get")
            .arg("claim")
            .arg(name)
            .arg("-n")
            .arg(namespace)
            .arg("-o")
            .arg("json")
            .output()
            .await
            .map_err(|e| DomainError::ExecutionFailed {
                reason: format!("Failed to query kubectl get claim: {}", e),
                details: None,
            })?;

        if !output.status.success() {
            return Err(DomainError::NotFound {
                entity: "CrossplaneClaim",
                id: format!("{}/{}", namespace, name),
            });
        }

        Ok(CrossplaneReconcileResult::new(
            name,
            "Claim",
            true,
            true,
            vec![],
            vec![],
        ))
    }

    async fn delete_claim(&self, namespace: &str, name: &str) -> Result<(), DomainError> {
        let output = Command::new(&self.kubectl_bin)
            .arg("delete")
            .arg("claim")
            .arg(name)
            .arg("-n")
            .arg(namespace)
            .output()
            .await
            .map_err(|e| DomainError::ExecutionFailed {
                reason: format!("Failed to spawn kubectl delete: {}", e),
                details: None,
            })?;

        if !output.status.success() {
            return Err(DomainError::ExecutionFailed {
                reason: "kubectl delete claim failed".to_string(),
                details: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            });
        }
        Ok(())
    }

    async fn list_managed_resources(
        &self,
        namespace: Option<&str>,
    ) -> Result<Vec<CrossplaneResource>, DomainError> {
        let mut cmd = Command::new(&self.kubectl_bin);
        cmd.arg("get").arg("managed").arg("-o").arg("json");
        if let Some(ns) = namespace {
            cmd.arg("-n").arg(ns);
        } else {
            cmd.arg("-A");
        }

        let output = cmd.output().await.map_err(|e| DomainError::ExecutionFailed {
            reason: format!("Failed to spawn kubectl get managed: {}", e),
            details: None,
        })?;

        if !output.status.success() {
            return Err(DomainError::ExecutionFailed {
                reason: "kubectl get managed failed".to_string(),
                details: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            });
        }

        Ok(vec![])
    }

    async fn list_providers(&self) -> Result<Vec<CrossplaneProviderStatus>, DomainError> {
        let output = Command::new(&self.kubectl_bin)
            .arg("get")
            .arg("providers")
            .arg("-o")
            .arg("json")
            .output()
            .await
            .map_err(|e| DomainError::ExecutionFailed {
                reason: format!("Failed to spawn kubectl get providers: {}", e),
                details: None,
            })?;

        if !output.status.success() {
            return Err(DomainError::ExecutionFailed {
                reason: "kubectl get providers failed".to_string(),
                details: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            });
        }

        Ok(vec![])
    }

    async fn render_composition(
        &self,
        claim_path: &Path,
        composition_path: &Path,
    ) -> Result<Vec<CrossplaneResource>, DomainError> {
        let output = Command::new(&self.crossplane_bin)
            .arg("beta")
            .arg("render")
            .arg(claim_path)
            .arg(composition_path)
            .output()
            .await
            .map_err(|e| DomainError::ExecutionFailed {
                reason: format!("Failed to spawn crossplane beta render: {}", e),
                details: None,
            })?;

        if !output.status.success() {
            return Err(DomainError::ExecutionFailed {
                reason: "crossplane beta render failed".to_string(),
                details: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            });
        }

        Ok(vec![])
    }
}
