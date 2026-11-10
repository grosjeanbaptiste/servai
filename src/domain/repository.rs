use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::errors::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RepoId(Uuid);

impl RepoId {
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

impl Default for RepoId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for RepoId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GitRef {
    Branch(String),
    Tag(String),
    Commit(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepoKind {
    Playbook,
    DockerCompose,
    ShellManifest,
    RawConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncStatus {
    Pending,
    Syncing,
    Synced,
    OutOfSync,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedRepository {
    id: RepoId,
    name: String,
    url: String,
    target_ref: GitRef,
    kind: RepoKind,
    sync_status: SyncStatus,
    last_commit_hash: Option<String>,
}

impl ManagedRepository {
    pub fn new(
        id: RepoId,
        name: String,
        url: String,
        target_ref: GitRef,
        kind: RepoKind,
    ) -> Result<Self, DomainError> {
        let trimmed_name = name.trim();
        if trimmed_name.is_empty() {
            return Err(DomainError::ValidationError(
                "Repository name cannot be empty".to_string(),
            ));
        }

        let trimmed_url = url.trim();
        if !trimmed_url.starts_with("git@")
            && !trimmed_url.starts_with("https://")
            && !trimmed_url.starts_with("http://")
            && !trimmed_url.starts_with("ssh://")
            && !trimmed_url.starts_with("file://")
        {
            return Err(DomainError::ValidationError(
                "Repository URL must be a valid git/https/ssh/file URL".to_string(),
            ));
        }

        Ok(Self {
            id,
            name: trimmed_name.to_string(),
            url: trimmed_url.to_string(),
            target_ref,
            kind,
            sync_status: SyncStatus::Pending,
            last_commit_hash: None,
        })
    }

    pub fn id(&self) -> RepoId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn target_ref(&self) -> &GitRef {
        &self.target_ref
    }

    pub fn kind(&self) -> RepoKind {
        self.kind
    }

    pub fn sync_status(&self) -> SyncStatus {
        self.sync_status
    }

    pub fn last_commit_hash(&self) -> Option<&str> {
        self.last_commit_hash.as_deref()
    }

    pub fn set_sync_status(&mut self, status: SyncStatus) {
        self.sync_status = status;
    }

    pub fn set_last_commit_hash(&mut self, hash: impl Into<String>) {
        self.last_commit_hash = Some(hash.into());
    }
}
