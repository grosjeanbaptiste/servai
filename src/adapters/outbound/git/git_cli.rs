use async_trait::async_trait;
use std::path::PathBuf;
use tokio::process::Command;

use crate::domain::{
    errors::DomainError,
    orchestration::OrchestrationStep,
    repository::{DriftReport, GitRef, ManagedRepository, RepoKind},
    server::ServerId,
};
use crate::ports::outbound::git_provider::GitProviderPort;

pub struct GitCliProvider {
    work_dir: PathBuf,
}

impl GitCliProvider {
    pub fn new(work_dir: impl Into<PathBuf>) -> Self {
        Self {
            work_dir: work_dir.into(),
        }
    }
}

impl Default for GitCliProvider {
    fn default() -> Self {
        Self::new("/tmp/servai/repos")
    }
}

#[async_trait]
impl GitProviderPort for GitCliProvider {
    async fn sync_repository(&self, repo: &ManagedRepository) -> Result<String, DomainError> {
        let repo_path = self.work_dir.join(repo.id().to_string());
        tokio::fs::create_dir_all(&self.work_dir).await.map_err(|e| {
            DomainError::VcsError(format!("Failed to create workspace directory: {}", e))
        })?;

        if !repo_path.exists() {
            let output = Command::new("git")
                .arg("clone")
                .arg(repo.url())
                .arg(&repo_path)
                .output()
                .await
                .map_err(|e| DomainError::VcsError(format!("Failed to run git clone: {}", e)))?;

            if !output.status.success() {
                return Err(DomainError::VcsError(
                    String::from_utf8_lossy(&output.stderr).to_string(),
                ));
            }
        } else {
            let fetch_output = Command::new("git")
                .current_dir(&repo_path)
                .arg("fetch")
                .arg("--all")
                .output()
                .await
                .map_err(|e| DomainError::VcsError(format!("Failed to run git fetch: {}", e)))?;

            if !fetch_output.status.success() {
                return Err(DomainError::VcsError(
                    String::from_utf8_lossy(&fetch_output.stderr).to_string(),
                ));
            }
        }

        // Checkout target ref
        let target = match repo.target_ref() {
            GitRef::Branch(b) => b.clone(),
            GitRef::Tag(t) => t.clone(),
            GitRef::Commit(c) => c.clone(),
        };

        let checkout_output = Command::new("git")
            .current_dir(&repo_path)
            .arg("checkout")
            .arg(&target)
            .output()
            .await
            .map_err(|e| DomainError::VcsError(format!("Failed to checkout ref: {}", e)))?;

        if !checkout_output.status.success() {
            return Err(DomainError::VcsError(
                String::from_utf8_lossy(&checkout_output.stderr).to_string(),
            ));
        }

        // Get latest commit hash
        let hash_output = Command::new("git")
            .current_dir(&repo_path)
            .arg("rev-parse")
            .arg("HEAD")
            .output()
            .await
            .map_err(|e| DomainError::VcsError(format!("Failed to rev-parse HEAD: {}", e)))?;

        if !hash_output.status.success() {
            return Err(DomainError::VcsError(
                String::from_utf8_lossy(&hash_output.stderr).to_string(),
            ));
        }

        Ok(String::from_utf8_lossy(&hash_output.stdout).trim().to_string())
    }

    async fn resolve_plan(&self, repo: &ManagedRepository) -> Result<Vec<OrchestrationStep>, DomainError> {
        match repo.kind() {
            RepoKind::Playbook => Ok(vec![
                OrchestrationStep::new("ansible_syntax_check", "ansible-playbook --syntax-check site.yml"),
                OrchestrationStep::new("ansible_apply", "ansible-playbook -i inventory site.yml"),
            ]),
            RepoKind::DockerCompose => Ok(vec![
                OrchestrationStep::new("docker_compose_pull", "docker compose pull"),
                OrchestrationStep::new("docker_compose_up", "docker compose up -d --remove-orphans"),
            ]),
            RepoKind::ShellManifest => Ok(vec![
                OrchestrationStep::new("run_manifest", "bash deploy.sh"),
            ]),
            RepoKind::RawConfig => Ok(vec![
                OrchestrationStep::new("rsync_configs", "rsync -avz ./ /etc/servai/"),
                OrchestrationStep::new("reload_services", "systemctl reload-or-restart servai-services"),
            ]),
            RepoKind::OpenTofu => Ok(vec![
                OrchestrationStep::new("opentofu_init", "tofu init -no-color"),
                OrchestrationStep::new("opentofu_plan", "tofu plan -no-color"),
                OrchestrationStep::new("opentofu_apply", "tofu apply -auto-approve -no-color"),
            ]),
            RepoKind::Crossplane => Ok(vec![
                OrchestrationStep::new("crossplane_render", "crossplane beta render claim.yaml composition.yaml"),
                OrchestrationStep::new("crossplane_apply", "kubectl apply -f claim.yaml"),
            ]),
        }
    }

    async fn detect_drift(
        &self,
        _server_id: &ServerId,
        repo: &ManagedRepository,
    ) -> Result<DriftReport, DomainError> {
        let repo_path = self.work_dir.join(repo.id().to_string());
        if !repo_path.exists() {
            return Ok(DriftReport::new(vec![], vec![], vec![]));
        }

        let output = Command::new("git")
            .current_dir(&repo_path)
            .arg("status")
            .arg("--porcelain")
            .output()
            .await
            .map_err(|e| DomainError::VcsError(format!("Failed to run git status: {}", e)))?;

        let text = String::from_utf8_lossy(&output.stdout);
        let mut modified = Vec::new();
        let mut untracked = Vec::new();
        let mut deleted = Vec::new();

        for line in text.lines() {
            if line.starts_with(" M") || line.starts_with("M ") {
                modified.push(line[3..].trim().to_string());
            } else if line.starts_with("??") {
                untracked.push(line[3..].trim().to_string());
            } else if line.starts_with(" D") || line.starts_with("D ") {
                deleted.push(line[3..].trim().to_string());
            }
        }

        Ok(DriftReport::new(modified, untracked, deleted))
    }
}
